#[derive(Template)]
#[template(path = "pages/hx-usernav.html")]
struct HXUsernavTemplate {
    user: User,
    config: Config,
    locale: RequestLocale,
}
async fn hx_usernav(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let try_user = get_user_login(headers, &db, redis.clone()).await;
    if let Some(user) = try_user {
        let template = HXUsernavTemplate { user, config, locale };
        Html(minifi_html(template.render().unwrap()))
    } else {
        let login_text = locale.t("nav-login");
        let result = format!("<a href=\"/login\"><button class=\"btn text-white\"><i class=\"fa-solid fa-user mx-2\" preload=\"mouseover\"></i>{}</button></a>", escape_html(&login_text));
        Html(minifi_html(result))
    }
}
