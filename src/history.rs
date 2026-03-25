#[derive(Template)]
#[template(path = "pages/history.html", escape = "none")]
struct HistoryTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}

async fn history(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "history".to_owned());
    let common_headers = extract_common_headers(&headers);
    let template = HistoryTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-history-items.html", escape = "none")]
struct HXHistoryItemsTemplate {
    media: Vec<Medium>,
    current_medium_id: String,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}

async fn hx_history(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
) -> axum::response::Html<Vec<u8>> {
    hx_history_inner(config, headers, db, redis, 0).await
}

async fn hx_history_page(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_history_inner(config, headers, db, redis, page).await
}

async fn hx_history_inner(
    config: Config,
    headers: HeaderMap,
    db: ScyllaDb,
    redis: RedisConn,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = match get_user_login(headers, &db, redis.clone()).await {
        Some(user) => user,
        None => {
            return Html(
                "Please log in to see your history"
                    .as_bytes()
                    .to_vec(),
            )
        }
    };

    let per_page: i64 = 30;
    // Fetch enough rows to cover offset + page + 1 (to detect has_more)
    let fetch_limit = ((page + 1) * per_page + 1) as i32;
    let offset = (page * per_page) as usize;

    let history_rows: Vec<(String, i64)> = db.session.execute_unpaged(&db.get_view_history, (&user.login, &fetch_limit))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i64)>().unwrap().filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Skip to the right page offset
    let page_rows: Vec<(String, i64)> = history_rows.into_iter().skip(offset).take(per_page as usize + 1).collect();
    let has_more = page_rows.len() > per_page as usize;

    // Fetch media details for each history entry
    let mut media: Vec<Medium> = Vec::new();
    for (media_id, viewed_at) in page_rows.iter().take(per_page as usize) {
        let media_row = db.session.execute_unpaged(&db.get_media_basic, (media_id,))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

        if let Some((id, name, owner, _visibility, _restricted_to_group, media_type)) = media_row {
            // Get view count
            let views: i64 = db.session.execute_unpaged(&db.get_view_count, (&id,))
                .await.ok().and_then(|r| r.into_rows_result().ok())
                .and_then(|rows| rows.maybe_first_row::<(scylla::value::CqlValue,)>().ok().flatten())
                .map(|r| match r.0 {
                    scylla::value::CqlValue::Counter(c) => c.0,
                    _ => 0,
                }).unwrap_or(0);

            let visit_time = DateTime::from_timestamp_millis(*viewed_at)
                .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string());

            media.push(Medium {
                id,
                name,
                owner,
                views,
                r#type: media_type,
                sprite_filename: None,
                sprite_x: 0,
                sprite_y: 0,
                visit_time,
            });
        }
    }

    let next_url = format!("/hx/history/{}", page + 1);

    let template = HXHistoryItemsTemplate {
        media,
        current_medium_id: String::new(),
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}
