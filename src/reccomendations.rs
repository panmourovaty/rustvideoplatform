async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let group_ids = get_user_group_ids(&pool, &user_login, redis.clone()).await;

    let media: Vec<Medium> = sqlx::query(
        "SELECT id, name, owner, views, type FROM media WHERE visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group = ANY($1)) LIMIT 20;"
    )
    .bind(&group_ids)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        Medium {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            views: row.get("views"),
            r#type: row.get("type"),
        }
    })
    .fetch_all(&pool)
    .await
    .map_err(|_| {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to fetch recommendations".into())
            .unwrap()
    })?;

    let template = HXMediumListTemplate {
        current_medium_id: mediumid,
        media,
        config
    };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}
