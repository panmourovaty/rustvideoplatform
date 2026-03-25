#[derive(Serialize, Deserialize, Debug, Clone)]
struct MeiliMedia {
    id: String,
    name: String,
    owner: String,
    views: i64,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    dislikes: i64,
    r#type: String,
    upload: i64,
    #[serde(default)]
    public: bool,
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
            sprite_filename: None,
            sprite_x: 0,
            sprite_y: 0,
            visit_time: None,
        }
    }
}

// --- Visibility filter helpers ---

/// Build a Meilisearch filter string that respects group-based visibility.
/// For logged-in users, shows public media + restricted media in their groups.
/// For anonymous users, shows only public media.
async fn build_visibility_filter(db: &ScyllaDb, user: &Option<User>) -> String {
    if let Some(u) = user {
        let group_ids: Vec<String> = db.session.execute_unpaged(&db.get_groups_for_user, (&u.login,))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).map(|r| r.0).collect())
            .unwrap_or_default();

        // Fetch channels the user is subscribed to
        let subscribed_channels: Vec<String> = db.session.execute_unpaged(&db.get_subscriptions, (&u.login,))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).map(|r| r.0).collect())
            .unwrap_or_default();

        // Build the list of group IDs + system group for all registered users
        let mut all_group_ids = group_ids;
        all_group_ids.push(SYSTEM_GROUP_ALL_REGISTERED.to_owned());

        let group_list = all_group_ids
            .iter()
            .map(|g| format!("'{}'", g.replace('\'', "")))
            .collect::<Vec<_>>()
            .join(", ");

        let mut filter = format!(
            "visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN [{}])",
            group_list
        );

        // For subscribers-only content, we need to check if the user is subscribed to the owner
        // Meilisearch doesn't support subqueries, so we add owner-based filters
        if !subscribed_channels.is_empty() {
            let owner_list = subscribed_channels
                .iter()
                .map(|o| format!("'{}'", o.replace('\'', "")))
                .collect::<Vec<_>>()
                .join(", ");

            filter = format!(
                "{} OR (visibility = 'restricted' AND restricted_to_group = '{}' AND owner IN [{}])",
                filter, SYSTEM_GROUP_SUBSCRIBERS, owner_list
            );
        }

        filter
    } else {
        "visibility = 'public'".to_owned()
    }
}

// --- Search suggestions (navbar autocomplete) ---

#[derive(Serialize, Deserialize)]
struct HXSearch {
    search: String,
}

#[derive(Debug, Clone)]
struct SuggestionUser {
    login: String,
    name: String,
    highlighted_name: String,
    profile_picture: Option<String>,
}

#[derive(Debug, Clone)]
struct SuggestionList {
    id: String,
    name: String,
    highlighted_name: String,
    owner: String,
    item_count: i64,
}

#[derive(Template)]
#[template(path = "pages/hx-searchsuggestions.html", escape = "none")]
struct HXSearchSuggestionsTemplate {
    users: Vec<SuggestionUser>,
    lists: Vec<SuggestionList>,
    media: Vec<Medium>,
    current_medium_id: String,
    config: Config,
}

async fn hx_search_suggestions(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    headers: HeaderMap,
    Form(form): Form<HXSearch>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let user = get_user_login(headers, &db, redis).await;
    let visibility_filter = build_visibility_filter(&db, &user).await;
    let vis_filter_for_lists = format!("({})", visibility_filter);

    let (users_res, lists_res, media_res) = tokio::join!(
        async {
            meili.index("users")
                .search()
                .with_query(&form.search)
                .with_limit(3)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name", "login"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "login", "name", "profile_picture",
                ]))
                .execute::<MeiliUser>()
                .await
        },
        async {
            meili.index("lists")
                .search()
                .with_query(&form.search)
                .with_filter(&vis_filter_for_lists)
                .with_limit(3)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "id", "name", "owner", "item_count",
                ]))
                .execute::<MeiliList>()
                .await
        },
        async {
            meili.index("media")
                .search()
                .with_query(&form.search)
                .with_filter(&visibility_filter)
                .with_limit(3)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "id", "name", "owner", "views", "type", "upload",
                ]))
                .execute::<MeiliMedia>()
                .await
        },
    );

    let users: Vec<SuggestionUser> = match users_res {
        Ok(r) => r.hits.into_iter().map(|hit| {
            let highlighted_name = hit
                .formatted_result
                .as_ref()
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&hit.result.name)
                .to_owned();
            SuggestionUser {
                login: hit.result.login,
                name: hit.result.name,
                highlighted_name,
                profile_picture: hit.result.profile_picture,
            }
        }).collect(),
        Err(e) => { eprintln!("Meilisearch users suggestion error: {:?}", e); vec![] }
    };

    let lists: Vec<SuggestionList> = match lists_res {
        Ok(r) => r.hits.into_iter().map(|hit| {
            let highlighted_name = hit
                .formatted_result
                .as_ref()
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&hit.result.name)
                .to_owned();
            SuggestionList {
                id: hit.result.id,
                name: hit.result.name,
                highlighted_name,
                owner: hit.result.owner,
                item_count: hit.result.item_count,
            }
        }).collect(),
        Err(e) => { eprintln!("Meilisearch lists suggestion error: {:?}", e); vec![] }
    };

    let media: Vec<Medium> = match media_res {
        Ok(r) => r.hits.into_iter().map(|hit| Medium::from(hit.result)).collect(),
        Err(e) => { eprintln!("Meilisearch media suggestion error: {:?}", e); vec![] }
    };

    if users.is_empty() && lists.is_empty() && media.is_empty() {
        return Html(
            "<li class=\"suggestion-empty\"><i class=\"fa-solid fa-circle-info me-2\"></i>No results found</li>"
                .to_owned(),
        );
    }

    let template = HXSearchSuggestionsTemplate {
        users,
        lists,
        media,
        current_medium_id: String::new(),
        config,
    };
    Html(template.render().unwrap())
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
    #[serde(default)]
    search_in: Option<String>,
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

// --- List search ---

/// Meilisearch document shape for the "lists" index.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MeiliList {
    id: String,
    name: String,
    owner: String,
    #[serde(default)]
    visibility: String,
    #[serde(default)]
    restricted_to_group: Option<String>,
    #[serde(default)]
    item_count: i64,
    #[serde(default)]
    created: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ListSearchHit {
    id: String,
    name: String,
    highlighted_name: String,
    owner: String,
    item_count: i64,
}

#[derive(Template)]
#[template(path = "pages/hx-search-lists.html", escape = "none")]
struct HXSearchListsTemplate {
    search_results: Vec<ListSearchHit>,
    next_page: usize,
    search_term: String,
    total_hits: usize,
    query_time_ms: usize,
    is_first_page: bool,
    has_more: bool,
    config: Config,
}

// --- User search ---

/// Meilisearch document shape for the "users" index.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MeiliUser {
    login: String,
    name: String,
    #[serde(default)]
    profile_picture: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct UserSearchHit {
    login: String,
    name: String,
    highlighted_name: String,
    profile_picture: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/hx-search-users.html", escape = "none")]
struct HXSearchUsersTemplate {
    search_results: Vec<UserSearchHit>,
    next_page: usize,
    search_term: String,
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

// --- Combined "all" search ---

#[derive(Template)]
#[template(path = "pages/hx-search-all.html", escape = "none")]
struct HXSearchAllTemplate {
    users: Vec<UserSearchHit>,
    lists: Vec<ListSearchHit>,
    media: Vec<MeiliSearchHit>,
    users_total: usize,
    lists_total: usize,
    media_total: usize,
    query_time_ms: usize,
    search_term: String,
    config: Config,
}

async fn hx_search(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    headers: HeaderMap,
    Path(pageid): Path<usize>,
    Form(form): Form<HXSearchForm>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let search_in = form.search_in.clone().unwrap_or_default();
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;

    match search_in.as_str() {
        "lists" => {
            return hx_search_lists_inner(config, db, meili, user, pageid, form.search).await;
        }
        "users" => {
            return hx_search_users_inner(config, meili, pageid, form.search).await;
        }
        "media" => {} // fall through to media-only Meilisearch
        _ => {
            // Default: search all types simultaneously
            return hx_search_all_inner(config, db, meili, user, form.search).await;
        }
    }

    let hits_per_page: usize = 41;
    let offset = pageid * 40;
    let next_page = pageid + 1;

    let media_type = form.media_type.clone().unwrap_or_default();
    let sort_by = form.sort_by.clone().unwrap_or_default();
    let date_range = form.date_range.clone().unwrap_or_default();

    // Build filter string with visibility-aware access control
    let visibility_filter = build_visibility_filter(&db, &user).await;
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

// --- List search inner (Meilisearch) ---

async fn hx_search_lists_inner(
    config: Config,
    db: ScyllaDb,
    meili: Arc<MeilisearchClient>,
    user: Option<User>,
    pageid: usize,
    search_term: String,
) -> axum::response::Html<String> {
    let hits_per_page: usize = 41;
    let offset = pageid * 40;

    // Reuse the same visibility filter logic as media search
    let visibility_filter = build_visibility_filter(&db, &user).await;
    let filter_str = format!("({})", visibility_filter);

    let index = meili.index("lists");
    let results = index
        .search()
        .with_query(&search_term)
        .with_filter(&filter_str)
        .with_offset(offset)
        .with_limit(hits_per_page)
        .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
        .with_highlight_pre_tag("<mark>")
        .with_highlight_post_tag("</mark>")
        .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
            "id", "name", "owner", "item_count",
        ]))
        .execute::<MeiliList>()
        .await;

    match results {
        Ok(search_results) => {
            let total_hits = search_results.estimated_total_hits.unwrap_or(0);
            let query_time_ms = search_results.processing_time_ms;

            let mut hits: Vec<ListSearchHit> = search_results
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
                    ListSearchHit {
                        id: hit.result.id,
                        name: hit.result.name,
                        highlighted_name,
                        owner: hit.result.owner,
                        item_count: hit.result.item_count,
                    }
                })
                .collect();

            let has_more = hits.len() == 41;
            if has_more {
                hits.truncate(40);
            }

            if hits.is_empty() && pageid == 0 {
                return Html(
                    "<div class=\"search-empty\"><i class=\"fa-solid fa-magnifying-glass fa-3x mb-3\"></i><h4>No results found</h4><p class=\"text-secondary\">Try different keywords</p></div>"
                        .to_owned(),
                );
            }

            if hits.is_empty() {
                return Html(
                    "<div class=\"col-12 text-center my-4\"><p class=\"text-secondary\"><i class=\"fa-solid fa-circle-check me-2\"></i>You have reached the end.</p></div>"
                        .to_owned(),
                );
            }

            let template = HXSearchListsTemplate {
                search_results: hits,
                next_page: pageid + 1,
                search_term,
                total_hits,
                query_time_ms,
                is_first_page: pageid == 0,
                has_more,
                config,
            };
            Html(template.render().unwrap())
        }
        Err(e) => {
            eprintln!("Meilisearch list search error: {:?}", e);
            Html(
                "<div class=\"search-empty\"><i class=\"fa-solid fa-triangle-exclamation fa-3x mb-3\"></i><h4>Search unavailable</h4><p class=\"text-secondary\">Please try again later</p></div>"
                    .to_owned(),
            )
        }
    }
}

// --- User search inner (Meilisearch) ---

async fn hx_search_users_inner(
    config: Config,
    meili: Arc<MeilisearchClient>,
    pageid: usize,
    search_term: String,
) -> axum::response::Html<String> {
    let hits_per_page: usize = 41;
    let offset = pageid * 40;

    let index = meili.index("users");
    let results = index
        .search()
        .with_query(&search_term)
        .with_offset(offset)
        .with_limit(hits_per_page)
        .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name", "login"]))
        .with_highlight_pre_tag("<mark>")
        .with_highlight_post_tag("</mark>")
        .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
            "login", "name", "profile_picture",
        ]))
        .execute::<MeiliUser>()
        .await;

    match results {
        Ok(search_results) => {
            let total_hits = search_results.estimated_total_hits.unwrap_or(0);
            let query_time_ms = search_results.processing_time_ms;

            let mut hits: Vec<UserSearchHit> = search_results
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
                    UserSearchHit {
                        login: hit.result.login,
                        name: hit.result.name,
                        highlighted_name,
                        profile_picture: hit.result.profile_picture,
                    }
                })
                .collect();

            let has_more = hits.len() == 41;
            if has_more {
                hits.truncate(40);
            }

            if hits.is_empty() && pageid == 0 {
                return Html(
                    "<div class=\"search-empty\"><i class=\"fa-solid fa-magnifying-glass fa-3x mb-3\"></i><h4>No results found</h4><p class=\"text-secondary\">Try different keywords</p></div>"
                        .to_owned(),
                );
            }

            if hits.is_empty() {
                return Html(
                    "<div class=\"col-12 text-center my-4\"><p class=\"text-secondary\"><i class=\"fa-solid fa-circle-check me-2\"></i>You have reached the end.</p></div>"
                        .to_owned(),
                );
            }

            let template = HXSearchUsersTemplate {
                search_results: hits,
                next_page: pageid + 1,
                search_term,
                total_hits,
                query_time_ms,
                is_first_page: pageid == 0,
                has_more,
                config,
            };
            Html(template.render().unwrap())
        }
        Err(e) => {
            eprintln!("Meilisearch user search error: {:?}", e);
            Html(
                "<div class=\"search-empty\"><i class=\"fa-solid fa-triangle-exclamation fa-3x mb-3\"></i><h4>Search unavailable</h4><p class=\"text-secondary\">Please try again later</p></div>"
                    .to_owned(),
            )
        }
    }
}

// --- Combined "all" search inner ---

async fn hx_search_all_inner(
    config: Config,
    db: ScyllaDb,
    meili: Arc<MeilisearchClient>,
    user: Option<User>,
    search_term: String,
) -> axum::response::Html<String> {
    let visibility_filter = build_visibility_filter(&db, &user).await;
    let vis_filter_parens = format!("({})", visibility_filter);

    let (users_res, lists_res, media_res) = tokio::join!(
        async {
            meili.index("users")
                .search()
                .with_query(&search_term)
                .with_limit(10)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name", "login"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "login", "name", "profile_picture",
                ]))
                .execute::<MeiliUser>()
                .await
        },
        async {
            meili.index("lists")
                .search()
                .with_query(&search_term)
                .with_filter(&vis_filter_parens)
                .with_limit(10)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "id", "name", "owner", "item_count",
                ]))
                .execute::<MeiliList>()
                .await
        },
        async {
            meili.index("media")
                .search()
                .with_query(&search_term)
                .with_filter(&visibility_filter)
                .with_limit(20)
                .with_attributes_to_highlight(meilisearch_sdk::search::Selectors::Some(&["name"]))
                .with_highlight_pre_tag("<mark>")
                .with_highlight_post_tag("</mark>")
                .with_attributes_to_retrieve(meilisearch_sdk::search::Selectors::Some(&[
                    "id", "name", "owner", "views", "likes", "type", "upload",
                ]))
                .execute::<MeiliMedia>()
                .await
        },
    );

    let (users, users_total, mut query_time_ms) = match users_res {
        Ok(r) => {
            let total = r.estimated_total_hits.unwrap_or(0);
            let time = r.processing_time_ms;
            let hits: Vec<UserSearchHit> = r.hits.into_iter().map(|hit| {
                let highlighted_name = hit.formatted_result.as_ref()
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&hit.result.name)
                    .to_owned();
                UserSearchHit {
                    login: hit.result.login,
                    name: hit.result.name,
                    highlighted_name,
                    profile_picture: hit.result.profile_picture,
                }
            }).collect();
            (hits, total, time)
        }
        Err(e) => { eprintln!("Meilisearch users search error: {:?}", e); (vec![], 0, 0) }
    };

    let (lists, lists_total) = match lists_res {
        Ok(r) => {
            let total = r.estimated_total_hits.unwrap_or(0);
            query_time_ms = query_time_ms.max(r.processing_time_ms);
            let hits: Vec<ListSearchHit> = r.hits.into_iter().map(|hit| {
                let highlighted_name = hit.formatted_result.as_ref()
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&hit.result.name)
                    .to_owned();
                ListSearchHit {
                    id: hit.result.id,
                    name: hit.result.name,
                    highlighted_name,
                    owner: hit.result.owner,
                    item_count: hit.result.item_count,
                }
            }).collect();
            (hits, total)
        }
        Err(e) => { eprintln!("Meilisearch lists search error: {:?}", e); (vec![], 0) }
    };

    let (media, media_total) = match media_res {
        Ok(r) => {
            let total = r.estimated_total_hits.unwrap_or(0);
            query_time_ms = query_time_ms.max(r.processing_time_ms);
            let hits: Vec<MeiliSearchHit> = r.hits.into_iter().map(|hit| {
                let highlighted_name = hit.formatted_result.as_ref()
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
            }).collect();
            (hits, total)
        }
        Err(e) => { eprintln!("Meilisearch media search error: {:?}", e); (vec![], 0) }
    };

    if users.is_empty() && lists.is_empty() && media.is_empty() {
        return Html(
            "<div class=\"search-empty\"><i class=\"fa-solid fa-magnifying-glass fa-3x mb-3\"></i><h4>No results found</h4><p class=\"text-secondary\">Try different keywords or adjust your filters</p></div>"
                .to_owned(),
        );
    }

    let template = HXSearchAllTemplate {
        users,
        lists,
        media,
        users_total,
        lists_total,
        media_total,
        query_time_ms,
        search_term,
        config,
    };
    Html(template.render().unwrap())
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
    let common_headers = extract_common_headers(&headers);
    let initial_query = params.q.unwrap_or_default();
    let template = SearchTemplate {
        sidebar,
        config,
        common_headers,
        initial_query,
    };
    Html(minifi_html(template.render().unwrap()))
}
