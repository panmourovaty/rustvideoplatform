#[derive(Template)]
#[template(path = "pages/home.html", escape = "none")]
struct HomeTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn home(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "home".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = HomeTemplate {
        config,
        sidebar,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}