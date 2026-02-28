#[derive(Serialize, Deserialize, Debug, Clone)]
struct MeiliMedia {
    id: String,
    name: String,
    owner: String,
    views: i64,
    likes: i64,
    r#type: String,
    upload: i64,
    #[serde(default)]
    visibility: String,
    #[serde(default)]
    restricted_to_group: Option<String>,
}

impl From<MeiliMedia> for Medium {
    fn from(media: MeiliMedia) -> Self {
        Medium {
            id: media.id,
            name: media.name,
            owner: media.owner,
            views: media.views,
            r#type: media.r#type,
        }
    }
}

// --- Visibility filter helpers ---

/// Build a Meilisearch filter string that respects group-based visibility.
/// For logged-in users, shows public media + restricted media in their groups.
/// For anonymous users, shows only public media.
async fn build_visibility_filter(pool: &PgPool, user: &Option<User>) -> String {
    if let Some(u) = user {
        let group_ids: Vec<String> = sqlx::query_scalar(
            "SELECT group_id FROM user_group_members WHERE user_login = $1"
        )
        .bind(&u.login)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if group_ids.is_empty() {
            return "visibility = 'public'".to_owned();
        }

        let group_list = group_ids
            .iter()
            .map(|g| format!("'{}'", g.replace('\'', "")))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN [{}])",
            group_list
        )
    } else {
        "visibility = 'public'".to_owned()
    }
}

// --- Search suggestions (navbar autocomplete) ---

#[derive(Serialize, Deserialize)]
struct HXSearch {
    search: String,
}

async fn hx_search_suggestions(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    headers: HeaderMap,
    Form(form): Form<HXSearch>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let user = get_user_login(headers, &pool, redis).await;
    let visibility_filter = build_visibility_filter(&pool, &user).await;

    let index = meili.index("media");
    let results = index
        .search()
        .with_query(&form.search)
        .with_filter(&visibility_filter)
        .with_limit(6)
        .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
        .with_highlight_pre_tag("<mark>")
        .with_highlight_post_tag("</mark>")
        .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
            "id", "name", "owner", "views", "likes", "type", "upload",
        ]))
        .execute::<MeiliMedia>()
        .await;

    match results {
        Ok(search_results) => {
            let media: Vec<Medium> = search_results
                .hits
                .into_iter()
                .map(|hit| hit.result.into())
                .collect();

            if media.is_empty() {
                return Html(
                    "<li class=\"suggestion-empty\"><i class=\"fa-solid fa-circle-info me-2\"></i>No results found</li>"
                        .to_owned(),
                );
            }

            let template = HXMediumListTemplate {
                current_medium_id: String::new(),
                media,
                config
            };
            Html(template.render().unwrap())
        }
        Err(e) => {
            eprintln!("Meilisearch search suggestion error: {:?}", e);
            Html(
                "<li class=\"suggestion-empty\"><i class=\"fa-solid fa-triangle-exclamation me-2\"></i>Search unavailable</li>"
                    .to_owned(),
            )
        }
    }
}

// --- Full search with filters ---

#[derive(Serialize, Deserialize, Debug)]
struct HXSearchForm {
    search: String,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    sort_by: Option<String>,
    #[serde(default)]
    date_range: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/hx-search.html", escape = "none")]
struct HXSearchTemplate {
    search_results: Vec<MeiliSearchHit>,
    next_page: usize,
    search_term: String,
    media_type: String,
    sort_by: String,
    date_range: String,
    total_hits: usize,
    query_time_ms: usize,
    is_first_page: bool,
    has_more: bool,
    config: Config,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MeiliSearchHit {
    id: String,
    name: String,
    highlighted_name: String,
    owner: String,
    views: i64,
    likes: i64,
    r#type: String,
    upload: i64,
}

async fn hx_search(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    headers: HeaderMap,
    Path(pageid): Path<usize>,
    Form(form): Form<HXSearchForm>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let hits_per_page: usize = 41;
    let offset = pageid * 40;
    let next_page = pageid + 1;

    let media_type = form.media_type.clone().unwrap_or_default();
    let sort_by = form.sort_by.clone().unwrap_or_default();
    let date_range = form.date_range.clone().unwrap_or_default();

    // Build filter string with visibility-aware access control
    let user = get_user_login(headers, &pool, redis).await;
    let visibility_filter = build_visibility_filter(&pool, &user).await;
    let mut filters: Vec<String> = vec![format!("({})", visibility_filter)];

    // Media type filter
    match media_type.as_str() {
        "video" => filters.push("type = \"video\"".to_owned()),
        "audio" => filters.push("type = \"audio\"".to_owned()),
        "picture" => filters.push("type = \"picture\"".to_owned()),
        _ => {} // "all" or empty - no type filter
    }

    // Date range filter
    let now = chrono::Utc::now().timestamp();
    match date_range.as_str() {
        "today" => filters.push(format!("upload > {}", now - 86400)),
        "week" => filters.push(format!("upload > {}", now - 604800)),
        "month" => filters.push(format!("upload > {}", now - 2592000)),
        "year" => filters.push(format!("upload > {}", now - 31536000)),
        _ => {} // "any" or empty - no date filter
    }

    let filter_str = filters.join(" AND ");

    // Build sort
    let sort_attrs: Vec<&str> = match sort_by.as_str() {
        "views" => vec!["views:desc"],
        "likes" => vec!["likes:desc"],
        "newest" => vec!["upload:desc"],
        "oldest" => vec!["upload:asc"],
        _ => vec![], // relevance - no sort override, Meilisearch default ranking
    };

    let index = meili.index("media");
    let mut query = index.search();
    query
        .with_query(&form.search)
        .with_filter(&filter_str)
        .with_offset(offset)
        .with_limit(hits_per_page)
        .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
        .with_highlight_pre_tag("<mark>")
        .with_highlight_post_tag("</mark>")
        .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
            "id", "name", "owner", "views", "likes", "type", "upload",
        ]));

    if !sort_attrs.is_empty() {
        query.with_sort(&sort_attrs);
    }

    let results = query.execute::<MeiliMedia>().await;

    match results {
        Ok(search_results) => {
            let total_hits = search_results.estimated_total_hits.unwrap_or(0);
            let query_time_ms = search_results.processing_time_ms;

            let mut hits: Vec<MeiliSearchHit> = search_results
                .hits
                .into_iter()
                .map(|hit| {
                    let highlighted_name = hit
                        .formatted_result
                        .as_ref()
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&hit.result.name)
                        .to_owned();

                    MeiliSearchHit {
                        id: hit.result.id,
                        name: hit.result.name,
                        highlighted_name,
                        owner: hit.result.owner,
                        views: hit.result.views,
                        likes: hit.result.likes,
                        r#type: hit.result.r#type,
                        upload: hit.result.upload,
                    }
                })
                .collect();

            let has_more = hits.len() == 41;
            if has_more {
                hits.truncate(40);
            }

            if hits.is_empty() && pageid == 0 {
                return Html(
                    "<div class=\"search-empty\"><i class=\"fa-solid fa-magnifying-glass fa-3x mb-3\"></i><h4>No results found</h4><p class=\"text-secondary\">Try different keywords or adjust your filters</p></div>"
                        .to_owned(),
                );
            }

            if hits.is_empty() {
                return Html(
                    "<div class=\"col-12 text-center my-4\"><p class=\"text-secondary\"><i class=\"fa-solid fa-circle-check me-2\"></i>You have reached the end.</p></div>"
                        .to_owned(),
                );
            }

            let template = HXSearchTemplate {
                search_results: hits,
                next_page,
                search_term: form.search,
                media_type,
                sort_by,
                date_range,
                total_hits,
                query_time_ms,
                is_first_page: pageid == 0,
                has_more,
                config
            };
            Html(template.render().unwrap())
        }
        Err(e) => {
            eprintln!("Meilisearch search error: {:?}", e);
            Html(
                "<div class=\"search-empty\"><i class=\"fa-solid fa-triangle-exclamation fa-3x mb-3\"></i><h4>Search unavailable</h4><p class=\"text-secondary\">Please try again later</p></div>"
                    .to_owned(),
            )
        }
    }
}

// --- Search page ---

#[derive(Template)]
#[template(path = "pages/search.html", escape = "none")]
struct SearchTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    initial_query: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: Option<String>,
}

async fn search(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "search".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let initial_query = params.q.unwrap_or_default();
    let template = SearchTemplate {
        sidebar,
        config,
        common_headers,
        initial_query,
    };
    Html(minifi_html(template.render().unwrap()))
}
