#[derive(Template)]
#[template(path = "pages/home.html", escape = "none")]
struct HomeTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    schema_org_json: String,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn home(
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let schema_org_json = serde_json::to_string(&serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": &config.instancename,
        "description": &config.description,
        "url": &config.site_url,
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{}/search?q={{search_term_string}}", config.site_url)
            },
            "query-input": "required name=search_term_string"
        }
    })).unwrap_or_default();
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "home".to_owned(), locale.clone());
    let template = HomeTemplate {
        config,
        sidebar,
        common_headers,
        schema_org_json,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}
