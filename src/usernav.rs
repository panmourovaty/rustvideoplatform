#[derive(Template)]
#[template(path = "pages/hx-usernav.html", escape = "none")]
struct HXUsernavTemplate {
    user: User,
}
async fn hx_usernav(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let try_user = get_user_login(headers, &pool, session_store).await;
    if try_user.is_some() {
        let user = try_user.unwrap();
        let template = HXUsernavTemplate { user };
        return Html(minifi_html(template.render().unwrap()));
    } else {
        let result = format!("<a href=\"/login\"><button class=\"btn text-white\"><i class=\"fa-solid fa-user mx-2\"></i>Log in</button></a>");
        return Html(minifi_html(result));
    }
}