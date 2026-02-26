#[derive(Template)]
#[template(path = "pages/trending.html", escape = "none")]
struct TrendingTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn trending(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "trending".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = TrendingTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_trending(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let group_ids = get_user_group_ids(&pool, &user_login, redis.clone()).await;

    let media: Vec<Medium> = sqlx::query(
        "SELECT id,name,owner,views,type FROM media WHERE visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group = ANY($1)) ORDER BY likes DESC LIMIT 100;"
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
    .expect("Database error");

    let template = HXMediumCardTemplate { media, config };
    Html(minifi_html(template.render().unwrap()))
}
