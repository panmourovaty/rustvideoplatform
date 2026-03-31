#[derive(Template)]
#[template(path = "pages/component-sidebar.html", escape = "none")]
struct SidebarComponentTemplate {
    config: Config,
    active_item: String,
    locale: RequestLocale,
}
fn generate_sidebar(config: &Config, active_item: String, locale: RequestLocale) -> String {
    let template = SidebarComponentTemplate {
        config: config.to_owned(),
        active_item,
        locale,
    };
    template.render().unwrap()
}

#[derive(Template)]
#[template(path = "pages/hx-sidebar.html", escape = "none")]
struct HXSidebarTemplate {
    active_item: String,
    locale: RequestLocale,
}
async fn hx_sidebar(
    Extension(redis): Extension<RedisConn>,
    Extension(db): Extension<ScyllaDb>,
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    Path(active_item): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;
    let common_headers = extract_common_headers(&headers);
    if user.is_some() {
        let locale = resolve_locale_noauth(
            common_headers.accept_language.as_deref(), &config.locale, &localization,
        );
        let template = HXSidebarTemplate { active_item, locale };
        Html(minifi_html(template.render().unwrap()))
    } else {
        Html("".as_bytes().to_vec())
    }
}
