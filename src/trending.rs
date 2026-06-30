#[derive(Template)]
#[template(path = "pages/trending.html")]
struct TrendingTemplate {
    sidebar: String,
    config: Config,
    schema_org_json: String,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn trending(
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let schema_org_json = json_for_html_script(&serde_json::json!({
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": format!("Trending - {}", config.instancename),
        "description": &config.description,
        "url": format!("{}/trending", config.site_url),
        "isPartOf": {
            "@type": "WebSite",
            "name": &config.instancename,
            "url": &config.site_url
        }
    }));
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "trending".to_owned(), locale.clone());
    let template = TrendingTemplate {
        sidebar,
        config,
        schema_org_json,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_trending(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, db, redis, localization, headers, 0).await
}

async fn hx_trending_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, db, redis, localization, headers, page).await
}

/// Try to load a page of trending media from the Redis cache.
/// Returns Some(media) if the cache is populated, None if cache is unavailable.
async fn try_trending_from_cache(redis: &mut RedisConn, offset: i64) -> Option<Vec<Medium>> {
    let exists: bool = redis.exists("cache:trending").await.ok()?;
    if !exists {
        return None;
    }

    let stop = offset + 30; // inclusive, fetches up to 31 items for has_more check
    let ids: Vec<String> = redis
        .zrevrange("cache:trending", offset as isize, stop as isize)
        .await
        .ok()?;

    if ids.is_empty() {
        return Some(Vec::new());
    }

    let sprite_filename: Option<String> = redis
        .get::<_, Option<String>>("cache:trending:sprite")
        .await
        .ok()
        .flatten();

    // Pipeline: fetch metadata for all IDs in a single round-trip
    let mut pipe = redis::pipe();
    for id in &ids {
        pipe.cmd("HGETALL")
            .arg(format!("cache:trending:info:{}", id));
    }
    let results: Vec<std::collections::HashMap<String, String>> =
        pipe.query_async(redis).await.ok()?;

    let mut media = Vec::with_capacity(ids.len());
    for (id, info) in ids.into_iter().zip(results) {
        if info.is_empty() {
            continue;
        }
        // Read sprite positions from cache (set by indexer based on actual sprite layout).
        // Only assign sprite_filename if positions are present (item is in the sprite).
        let has_sprite = info.contains_key("sprite_x");
        media.push(Medium {
            id,
            name: info.get("name").cloned().unwrap_or_default(),
            owner: info.get("owner").cloned().unwrap_or_default(),
            views: info
                .get("views")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            r#type: info.get("type").cloned().unwrap_or_default(),
            sprite_filename: if has_sprite { sprite_filename.clone() } else { None },
            sprite_x: info.get("sprite_x").and_then(|v| v.parse().ok()).unwrap_or(0),
            sprite_y: info.get("sprite_y").and_then(|v| v.parse().ok()).unwrap_or(0),
            visit_time: None,
        });
    }

    Some(media)
}

async fn hx_trending_inner(
    config: Config,
    db: ScyllaDb,
    mut redis: RedisConn,
    localization: Arc<LocalizationService>,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    if !valid_page(page) {
        return Html(Vec::new());
    }
    let offset = page * 30;

    // Try Redis cache first (pre-computed by the indexer)
    // Trending is cache-driven; the indexer populates this cache.
    let mut media: Vec<Medium> = try_trending_from_cache(&mut redis, offset)
        .await
        .unwrap_or_default();
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;
    let mut accessible_media = Vec::with_capacity(media.len());
    for item in media {
        let visibility = db
            .session
            .execute_unpaged(&db.get_media_basic, (&item.id,))
            .await
            .ok()
            .and_then(|result| result.into_rows_result().ok())
            .and_then(|rows| {
                rows.maybe_first_row::<(
                    String,
                    String,
                    String,
                    String,
                    Option<String>,
                    String,
                )>()
                .ok()
                .flatten()
            });
        let Some((_id, _name, owner, visibility, restricted_group, _media_type)) = visibility
        else {
            continue;
        };
        if can_access_restricted(
            &db,
            &visibility,
            restricted_group.as_deref(),
            &owner,
            &user,
            redis.clone(),
        )
        .await
        {
            accessible_media.push(item);
        }
    }
    media = accessible_media;

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/trending/{}", next_page);

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}
