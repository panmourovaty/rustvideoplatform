#[derive(Template)]
#[template(path = "pages/login.html", escape = "none")]
struct LoginTemplate {
    config: Config,
}
async fn login(Extension(config): Extension<Config>) -> axum::response::Html<Vec<u8>> {
    let template = LoginTemplate { config };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct LoginForm {
    login: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct User {
    login: String,
    name: String,
}

async fn hx_login(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let password_hash_get = sqlx::query!(
        "SELECT password_hash FROM users WHERE login=$1;",
        form.login
    )
    .fetch_one(&pool)
    .await;

    if password_hash_get.is_err() {
        let response_headers = HeaderMap::new();
        let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();

        return (StatusCode::OK, response_headers, response_body);
    }

    let password_hash = password_hash_get.unwrap().password_hash;

    if Argon2::default()
        .verify_password(
            form.password.as_bytes(),
            &PasswordHash::new(&password_hash).unwrap(),
        )
        .is_ok()
    {
        let session_cookie_value = generate_secure_string();
        let session_cookie_set = format!("session={}; Path=/", session_cookie_value);
        session_store
            .lock()
            .await
            .insert(session_cookie_value.clone(), form.login);

        let mut response_headers = HeaderMap::new();
        response_headers.insert("Set-Cookie", session_cookie_set.parse().unwrap());
        let response_body = "<b class=\"text-sucess\">LOGIN SUCESS</b><script>window.location.replace(\"/\");</script>".to_owned();
        return (StatusCode::OK, response_headers, response_body);
    } else {
        let response_headers = HeaderMap::new();
        let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();

        return (StatusCode::OK, response_headers, response_body);
    }
}

async fn hx_logout(
    headers: HeaderMap,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
) -> axum::response::Html<String> {
    let session_cookie = parse_cookie_header(headers.get("Cookie").unwrap().to_str().unwrap())
        .get("session")
        .unwrap()
        .to_owned();
    session_store.lock().await.remove_entry(&session_cookie);
    Html("<h1>LOGOUT SUCESS</h1><script>window.location.replace(\"/\");</script>".to_owned())
}