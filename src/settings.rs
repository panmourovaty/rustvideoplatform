#[derive(Template)]
#[template(path = "pages/settings.html")]
struct SettingsTemplate {
    sidebar: String,
    config: Config,
    current_user: Option<User>,
    active_tab: String,
    locale: RequestLocale,
    resolved_lang: String,
}

async fn settings(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "channel_name".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_password(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "password".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_profile_picture(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "profile_picture".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_channel_picture(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "channel_picture".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_diagnostics(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "diagnostics".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_2fa(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Response {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return axum::response::Redirect::to("/login").into_response();
    }
    axum::response::Redirect::to("/settings/password").into_response()
}

// --- HTMX tab content handlers ---

#[derive(Template)]
#[template(path = "pages/hx-settings-channel-name.html")]
struct HXSettingsChannelNameTemplate {
    current_name: String,
    locale: RequestLocale,
}
async fn hx_settings_channel_name(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXSettingsChannelNameTemplate {
        current_name: user_info.name,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct ChannelNameForm {
    channel_name: String,
}
async fn hx_settings_channel_name_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<ChannelNameForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let new_name = form.channel_name.trim();
    if new_name.is_empty() || new_name.len() > 100 {
        return Html(minifi_html("<b class=\"text-danger\">Channel name must be between 1 and 100 characters.</b>".to_owned()));
    }
    let result = db.session.execute_unpaged(&db.update_user_name, (new_name, &user_info.login)).await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update channel name.</b>".to_owned()));
    }
    Html(minifi_html(format!("<b class=\"text-success\">Channel name updated to \"{}\".</b>", askama::filters::escape(new_name, askama::filters::Html).unwrap())))
}

#[derive(Template)]
#[template(path = "pages/hx-settings-password.html")]
struct HXSettingsPasswordTemplate {
    locale: RequestLocale,
}
async fn hx_settings_password(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXSettingsPasswordTemplate { locale };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}
async fn hx_settings_password_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PasswordForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    if form.new_password != form.confirm_password {
        return Html(minifi_html("<b class=\"text-danger\">New passwords do not match.</b>".to_owned()));
    }
    if form.new_password.len() < 8 {
        return Html(minifi_html("<b class=\"text-danger\">New password must be at least 8 characters.</b>".to_owned()));
    }

    // Verify current password
    let stored_hash = db.session.execute_unpaged(&db.get_user_password, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());
    let stored_hash = match stored_hash {
        Some((hash,)) => hash,
        None => {
            return Html(minifi_html("<b class=\"text-danger\">Failed to verify current password.</b>".to_owned()));
        }
    };
    if Argon2::default()
        .verify_password(form.current_password.as_bytes(), &PasswordHash::new(&stored_hash).unwrap())
        .is_err()
    {
        return Html(minifi_html("<b class=\"text-danger\">Current password is incorrect.</b>".to_owned()));
    }

    // Hash new password with Argon2id
    let salt_bytes: [u8; 16] = rand::random();
    let salt = argon2::password_hash::SaltString::encode_b64(&salt_bytes).unwrap();
    let new_hash = Argon2::default()
        .hash_password(form.new_password.as_bytes(), &salt);
    if new_hash.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to hash new password.</b>".to_owned()));
    }
    let new_hash_string = new_hash.unwrap().to_string();

    let result = db.session.execute_unpaged(&db.update_user_password, (&new_hash_string, &user_info.login)).await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update password.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Password updated successfully.</b>".to_owned()))
}

// --- Profile Picture ---

#[derive(Serialize, Deserialize)]
struct PictureMedium {
    id: String,
    name: String,
    visibility: String,
}
#[derive(Template)]
#[template(path = "pages/hx-settings-profile-picture.html")]
struct HXSettingsProfilePictureTemplate {
    media: Vec<PictureMedium>,
    current_picture: Option<String>,
    config: Config,
    locale: RequestLocale,
}
async fn hx_settings_profile_picture(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    let current_picture: Option<String> = db.session.execute_unpaged(&db.get_user_profile_picture, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .map(|(pic,)| pic)
        .unwrap_or(None);

    let all_media = db.session.execute_unpaged(&db.get_media_by_owner, (&user_info.login, 1000i32)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>()
            .unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let media: Vec<PictureMedium> = all_media.into_iter()
        .filter(|(_id, _name, _desc, _views, mtype, _upload, visibility, _restricted)| {
            mtype == "picture" && (visibility == "public" || visibility == "hidden")
        })
        .map(|(id, name, _desc, _views, _mtype, _upload, visibility, _restricted)| {
            PictureMedium { id, name, visibility }
        })
        .collect();

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXSettingsProfilePictureTemplate { media, current_picture, config, locale };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PictureForm {
    medium_id: String,
}
async fn hx_settings_profile_picture_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PictureForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Verify the medium belongs to this user, is an image, and is public or hidden
    let medium = db.session.execute_unpaged(&db.get_media_basic, (&form.medium_id,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());
    match medium {
        Some((_id, _name, owner, visibility, _restricted_to_group, medium_type)) => {
            if owner != user_info.login {
                return Html(minifi_html("<b class=\"text-danger\">You can only use your own media.</b>".to_owned()));
            }
            if medium_type != "picture" {
                return Html(minifi_html("<b class=\"text-danger\">Only image media can be used as a profile picture.</b>".to_owned()));
            }
            if visibility != "public" && visibility != "hidden" {
                return Html(minifi_html("<b class=\"text-danger\">Media must be public or hidden.</b>".to_owned()));
            }
        }
        None => {
            return Html(minifi_html("<b class=\"text-danger\">Medium not found.</b>".to_owned()));
        }
    }

    let result = db.session.execute_unpaged(&db.update_user_profile_picture, (&form.medium_id, &user_info.login)).await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update profile picture.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Profile picture updated.</b>".to_owned()))
}

// --- Channel Picture ---

#[derive(Template)]
#[template(path = "pages/hx-settings-channel-picture.html")]
struct HXSettingsChannelPictureTemplate {
    media: Vec<PictureMedium>,
    current_picture: Option<String>,
    config: Config,
    locale: RequestLocale,
}
async fn hx_settings_channel_picture(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    let current_picture: Option<String> = db.session.execute_unpaged(&db.get_user_channel_picture, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .map(|(pic,)| pic)
        .unwrap_or(None);

    let all_media = db.session.execute_unpaged(&db.get_media_by_owner, (&user_info.login, 1000i32)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>()
            .unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let media: Vec<PictureMedium> = all_media.into_iter()
        .filter(|(_id, _name, _desc, _views, mtype, _upload, visibility, _restricted)| {
            mtype == "picture" && (visibility == "public" || visibility == "hidden")
        })
        .map(|(id, name, _desc, _views, _mtype, _upload, visibility, _restricted)| {
            PictureMedium { id, name, visibility }
        })
        .collect();

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXSettingsChannelPictureTemplate { media, current_picture, config, locale };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_settings_channel_picture_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PictureForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Verify the medium belongs to this user, is an image, and is public or hidden
    let medium = db.session.execute_unpaged(&db.get_media_basic, (&form.medium_id,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());
    match medium {
        Some((_id, _name, owner, visibility, _restricted_to_group, medium_type)) => {
            if owner != user_info.login {
                return Html(minifi_html("<b class=\"text-danger\">You can only use your own media.</b>".to_owned()));
            }
            if medium_type != "picture" {
                return Html(minifi_html("<b class=\"text-danger\">Only image media can be used as a channel picture.</b>".to_owned()));
            }
            if visibility != "public" && visibility != "hidden" {
                return Html(minifi_html("<b class=\"text-danger\">Media must be public or hidden.</b>".to_owned()));
            }
        }
        None => {
            return Html(minifi_html("<b class=\"text-danger\">Medium not found.</b>".to_owned()));
        }
    }

    let result = db.session.execute_unpaged(&db.update_user_channel_picture, (&form.medium_id, &user_info.login)).await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update channel picture.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Channel picture updated.</b>".to_owned()))
}

// --- Diagnostics ---

fn get_os_distro() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(pretty_name) = line.strip_prefix("PRETTY_NAME=") {
                return pretty_name.trim_matches('"').to_owned();
            }
        }
    }
    std::env::consts::OS.to_owned()
}

fn get_kernel_version() -> String {
    let output = std::process::Command::new("uname")
        .arg("-r")
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8(o.stdout).unwrap_or_default().trim().to_owned();
            if v.is_empty() { "unknown".to_owned() } else { v }
        }
        _ => "unknown".to_owned(),
    }
}

async fn get_scylla_version(db: &ScyllaDb) -> String {
    if let Some(v) = db.session
        .query_unpaged("SHOW VERSION", ())
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(v,)| v)
    {
        return v;
    }
    db.session
        .query_unpaged("SELECT release_version FROM system.local", ())
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(v,)| v)
        .unwrap_or_else(|| "unknown".to_owned())
}

async fn get_meilisearch_version(meili: &MeilisearchClient) -> String {
    meili.get_version()
        .await
        .map(|v| v.pkg_version)
        .unwrap_or_else(|_| "unknown".to_owned())
}

async fn get_redis_version(mut redis: RedisConn) -> String {
    let info: redis::RedisResult<String> = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut redis)
        .await;
    match info {
        Ok(info_str) => {
            for line in info_str.lines() {
                if let Some(version) = line.strip_prefix("redis_version:") {
                    return version.trim().to_owned();
                }
            }
            "unknown".to_owned()
        }
        Err(_) => "unknown".to_owned(),
    }
}

#[derive(Template)]
#[template(path = "pages/hx-settings-diagnostics.html")]
struct HXSettingsDiagnosticsTemplate {
    git_commit: String,
    git_branch: String,
    version: String,
    os_distro: String,
    os_kernel: String,
    os_arch: String,
    scylla_version: String,
    meilisearch_version: String,
    redis_version: String,
    locale: RequestLocale,
}

async fn hx_settings_diagnostics(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    let git_commit = env!("GIT_COMMIT_HASH").to_owned();
    let git_branch = env!("GIT_BRANCH").to_owned();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let os_distro = get_os_distro();
    let os_kernel = get_kernel_version();
    let os_arch = std::env::consts::ARCH.to_owned();
    let scylla_version = get_scylla_version(&db).await;
    let meilisearch_version = get_meilisearch_version(&meili).await;
    let redis_version = get_redis_version(redis.clone()).await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;

    let template = HXSettingsDiagnosticsTemplate {
        git_commit,
        git_branch,
        version,
        os_distro,
        os_kernel,
        os_arch,
        scylla_version,
        meilisearch_version,
        redis_version,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

// --- Theme ---

/// Serve the user's preferred CSS theme. Falls back to a redirect to /style.css for
/// unauthenticated users or those with the default theme.
async fn user_style_css(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> Response {
    if let Some(user) = get_user_login(headers, &db, redis).await {
        let theme: Option<String> = db.session.execute_unpaged(&db.get_user_theme, (&user.login,)).await
            .ok().and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
            .map(|(t,)| t)
            .unwrap_or(None);

        if let Some(ref name) = theme {
            if name != "default" && is_valid_theme_name(name) {
                let path = format!("source/system_style/{}.css", name);
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    return Response::builder()
                        .header("Content-Type", "text/css; charset=utf-8")
                        .body(Body::from(content))
                        .unwrap();
                }
            }
        }
    }

    Redirect::to("/style.css").into_response()
}

/// Only allow alphanumeric characters, hyphens, and underscores in theme names to
/// prevent path traversal attacks.
fn is_valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[derive(Template)]
#[template(path = "pages/hx-settings-theme.html")]
struct HXSettingsThemeTemplate {
    available_themes: Vec<String>,
    current_theme: String,
    locale: RequestLocale,
}

async fn hx_settings_theme(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let current_theme: String = db.session.execute_unpaged(&db.get_user_theme, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .and_then(|(t,)| t)
        .unwrap_or_else(|| "default".to_owned());

    let available_themes = list_available_themes().await;
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;

    let template = HXSettingsThemeTemplate {
        available_themes,
        current_theme,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct ThemeForm {
    theme: String,
}

async fn hx_settings_theme_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<ThemeForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let theme = form.theme.trim().to_owned();

    // "default" is always valid; otherwise validate name and check file exists
    if theme != "default" {
        if !is_valid_theme_name(&theme) {
            return Html(minifi_html(
                "<b class=\"text-danger\">Invalid theme name.</b>".to_owned(),
            ));
        }
        let path = format!("source/system_style/{}.css", theme);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Html(minifi_html(
                "<b class=\"text-danger\">Theme not found.</b>".to_owned(),
            ));
        }
    }

    let result = db.session.execute_unpaged(&db.update_user_theme, (&theme, &user_info.login)).await;

    if result.is_err() {
        return Html(minifi_html(
            "<b class=\"text-danger\">Failed to save theme preference.</b>".to_owned(),
        ));
    }

    Html(minifi_html(
        "<b class=\"text-success\">Theme updated. Reload the page to see the change.</b>"
            .to_owned(),
    ))
}

/// Return a sorted list of theme names (without the .css extension) found in
/// source/system_style/.
async fn list_available_themes() -> Vec<String> {
    let mut themes = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir("source/system_style").await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(stem) = name_str.strip_suffix(".css") {
                if is_valid_theme_name(stem) {
                    themes.push(stem.to_owned());
                }
            }
        }
    }
    themes.sort();
    themes
}

// Settings page handler for the theme tab URL
async fn settings_theme(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "theme".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

// --- Language settings ---

/// Fetch the user's preferred language from the database.
async fn get_user_language_pref(db: &ScyllaDb, login: &str) -> Option<String> {
    db.session
        .execute_unpaged(&db.get_user_language, (login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .and_then(|(lang,)| lang)
}

/// Resolve the request locale for a settings page render.
/// Looks up the user's saved language preference then falls back to browser
/// negotiation and finally to the server-wide config locale.
async fn resolve_request_locale(
    user: Option<&User>,
    common_headers: &CommonHeaders,
    db: &ScyllaDb,
    localization: &Arc<LocalizationService>,
    config: &Config,
) -> RequestLocale {
    let user_pref = if let Some(u) = user {
        get_user_language_pref(db, &u.login).await
    } else {
        None
    };

    let lang = localization.resolve_language(
        user_pref.as_deref(),
        common_headers.accept_language.as_deref(),
        &config.locale,
    );

    RequestLocale::new(lang.clone(), localization.clone())
}

#[derive(Template)]
#[template(path = "pages/hx-settings-language.html")]
struct HXSettingsLanguageTemplate {
    available_langs: Vec<LangInfo>,
    current_language: String,
    locale: RequestLocale,
}

async fn hx_settings_language(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let current_language = get_user_language_pref(&db, &user_info.login)
        .await
        .unwrap_or_else(|| "browser".to_owned());

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(
        Some(&user_info),
        &common_headers,
        &db,
        &localization,
        &config,
    )
    .await;

    let available_langs = localization.available_langs.clone();

    let template = HXSettingsLanguageTemplate {
        available_langs,
        current_language,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct LanguageForm {
    language: String,
}

async fn hx_settings_language_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Form(form): Form<LanguageForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let lang = form.language.trim().to_owned();

    // "browser" is the special value meaning "detect from Accept-Language".
    // Any other value must be a known language code.
    if lang != "browser" && !localization.is_valid_lang(&lang) {
        let common_headers = extract_common_headers(&headers);
        let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
        return Html(minifi_html(format!(
            "<b class=\"text-danger\">{}</b>",
            locale.t("language-invalid")
        )));
    }

    let result = db
        .session
        .execute_unpaged(&db.update_user_language, (&lang, &user_info.login))
        .await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;

    if result.is_err() {
        return Html(minifi_html(format!(
            "<b class=\"text-danger\">{}</b>",
            locale.t("language-save-failed")
        )));
    }

    Html(minifi_html(format!(
        "<b class=\"text-success\">{}</b>",
        locale.t("language-saved-success")
    )))
}

async fn settings_language(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(user_info.as_ref(), &common_headers, &db, &localization, &config).await;
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "settings".to_owned(),
        user_info.clone(),
        locale.clone(),
    );
    let template = SettingsTemplate {
        sidebar,
        config,
        current_user: user_info,
        active_tab: "language".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}
