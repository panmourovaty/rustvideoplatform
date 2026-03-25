#[derive(Template)]
#[template(path = "pages/trending.html", escape = "none")]
struct TrendingTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    schema_org_json: String,
}
async fn trending(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let schema_org_json = serde_json::to_string(&serde_json::json!({
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
    })).unwrap_or_default();
    let sidebar = generate_sidebar(&config, "trending".to_owned());
    let common_headers = extract_common_headers(&headers);
    let template = TrendingTemplate {
        sidebar,
        config,
        common_headers,
        schema_org_json,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_trending(
    Extension(config): Extension<Config>,
    Extension(redis): Extension<RedisConn>,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, redis, 0).await
}

async fn hx_trending_page(
    Extension(config): Extension<Config>,
    Extension(redis): Extension<RedisConn>,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, redis, page).await
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
    mut redis: RedisConn,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let offset = page * 30;

    // Try Redis cache first (pre-computed by the indexer)
    let mut media = match try_trending_from_cache(&mut redis, offset).await {
        Some(cached) => cached,
        None => {
            // Cache not available — trending is cache-driven, so return empty.
            // The indexer will populate the cache.
            Vec::new()
        }
    };

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/trending/{}", next_page);

    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}
