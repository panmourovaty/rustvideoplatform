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

#[derive(Template)]
#[template(path = "pages/hx-trending.html", escape = "none")]
struct HXTrendingTemplate {
    reccomendations: Vec<Medium>,
}
async fn hx_trending(Extension(pool): Extension<PgPool>) -> axum::response::Html<Vec<u8>> {
    let reccomendations = sqlx::query_as!(Medium,
        "SELECT id,name,owner,views,type FROM media WHERE public=true ORDER BY likes DESC LIMIT 100;"
    )
    .fetch_all(&pool)
    .await
    .expect("Nemohu provést dotaz");
    let template = HXTrendingTemplate { reccomendations };
    Html(minifi_html(template.render().unwrap()))
}