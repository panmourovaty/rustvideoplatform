#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    config: Config,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn login(
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let template = LoginTemplate {
        config,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct LoginForm {
    login: String,
    password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    login: String,
    name: String,
    profile_picture: Option<String>,
}

async fn hx_login(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(mut redis): Extension<RedisConn>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let rate_key = format!("ratelimit:login:{}", form.login.to_ascii_lowercase());
    if rate_limit_exceeded(
        &mut redis,
        &rate_key,
        LOGIN_ATTEMPT_LIMIT,
        LOGIN_ATTEMPT_WINDOW_SECONDS,
    )
    .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            HeaderMap::new(),
            "<b class=\"text-danger\">Too many login attempts. Try again later.</b>".to_owned(),
        );
    }

    let password_hash_result = db.session.execute_unpaged(&db.get_user_password, (&form.login,)).await;

    let password_hash = match password_hash_result
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
    {
        Some(row) => row.0,
        None => {
            let response_headers = HeaderMap::new();
            let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();
            return (StatusCode::OK, response_headers, response_body);
        }
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::OK,
                HeaderMap::new(),
                "<b class=\"text-danger\">Wrong user name or password</b>".to_owned(),
            );
        }
    };

    if Argon2::default()
        .verify_password(form.password.as_bytes(), &parsed_hash)
        .is_ok()
    {
        // Check if TOTP is enabled for this user
        let totp_enabled: bool = db.session.execute_unpaged(&db.get_user_totp_enabled, (&form.login,))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(Option<bool>,)>().ok().flatten())
            .map(|r| r.0.unwrap_or(false))
            .unwrap_or(false);

        if totp_enabled {
            // Create a short-lived pending session and ask for TOTP code
            let pending_token = generate_secure_string();
            let pending_result: Result<(), _> = redis
                .set_ex(
                    format!("pending_2fa:{}", pending_token),
                    &form.login,
                    300u64,
                )
                .await;
            if pending_result.is_err() {
                return (
                    StatusCode::OK,
                    HeaderMap::new(),
                    "<b class=\"text-danger\">Server error, please try again</b>".to_owned(),
                );
            }

            let response_body = "<p class=\"mt-2\">Enter your authenticator code:</p>\
                <form hx-post=\"/hx/login/2fa/totp\" hx-target=\"#logininfo\" hx-swap=\"innerHTML\">\
                <input type=\"hidden\" name=\"pending_token\" value=\"".to_owned()
                + &pending_token
                + "\">\
                <input class=\"form-control\" style=\"text-align:center\" type=\"text\" name=\"totp_code\"\
                 placeholder=\"000000\" maxlength=\"6\" pattern=\"[0-9]{6}\" autocomplete=\"one-time-code\"\
                 autofocus inputmode=\"numeric\">\
                <button class=\"btn btn-primary mt-3 w-100\" type=\"submit\">Verify Code</button>\
                </form>";
            return (StatusCode::OK, HeaderMap::new(), response_body);
        }

        let session_cookie_value =
            match create_authenticated_session(&mut redis, &form.login, &config).await {
                Ok(token) => token,
                Err(_) => {
                    return (
                        StatusCode::OK,
                        HeaderMap::new(),
                        "<b class=\"text-danger\">Server error, please try again</b>".to_owned(),
                    );
                }
            };
        let _: Result<(), _> = redis.del(&rate_key).await;

        let mut response_headers = HeaderMap::new();
        response_headers.insert("Set-Cookie", build_session_cookie(&session_cookie_value, &config).parse().unwrap());
        response_headers.insert("HX-Redirect", "/".parse().unwrap());
        (StatusCode::OK, response_headers, String::new())
    } else {
        let response_headers = HeaderMap::new();
        let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();

        (StatusCode::OK, response_headers, response_body)
    }
}

async fn hx_logout(
    headers: HeaderMap,
    Extension(config): Extension<Config>,
    Extension(mut redis): Extension<RedisConn>,
) -> impl IntoResponse {
    if let Some(session_cookie) = parse_all_cookies(&headers).get("session").cloned() {
        let _: () = redis
            .del(format!("session:{}", session_cookie))
            .await
            .unwrap_or(());
    }
    let mut response_headers = HeaderMap::new();
    if let Ok(cookie) = clear_session_cookie(&config).parse() {
        response_headers.insert(axum::http::header::SET_COOKIE, cookie);
    }
    response_headers.insert("HX-Redirect", axum::http::HeaderValue::from_static("/"));
    (StatusCode::OK, response_headers, String::new())
}
