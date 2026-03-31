#[derive(Template)]
#[template(path = "pages/hx-usernav.html", escape = "none")]
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
    if try_user.is_some() {
        let user = try_user.unwrap();
        let template = HXUsernavTemplate { user, config, locale };
        return Html(minifi_html(template.render().unwrap()));
    } else {
        let login_text = locale.t("nav-login");
        let result = format!("<a href=\"/login\"><button class=\"btn text-white\"><i class=\"fa-solid fa-user mx-2\" preload=\"mouseover\"></i>{}</button></a>", login_text);
        return Html(minifi_html(result));
    }
}
