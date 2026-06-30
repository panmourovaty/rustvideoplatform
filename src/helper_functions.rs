fn minifi_html(html: String) -> Vec<u8> {
    let config = minify_html_onepass::Cfg {
        minify_css: false,
        minify_js: false,
    };
    match minify_html_onepass::copy(html.as_bytes(), &config) {
        Ok(minified) => minified,
        Err(_) => html.into_bytes(),
    }
}

fn read_lines_to_vec(filepath: &str) -> Vec<String> {
    let Ok(file) = std::fs::File::open(filepath) else {
        return Vec::new();
    };
    let reader = std::io::BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .collect();

    lines
}

fn generate_secure_string() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const STRING_LEN: usize = 100;

    (0..STRING_LEN)
        .map(|_| {
            let idx = rand::random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn is_valid_resource_id(value: &str) -> bool {
    (3..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
}

fn normalize_resource_id(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    is_valid_resource_id(&normalized).then_some(normalized)
}

fn is_valid_subtitle_label(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 100
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn sanitize_search_highlight(value: &str) -> String {
    escape_html(value)
        .replace("&lt;mark&gt;", "<mark>")
        .replace("&lt;/mark&gt;", "</mark>")
}

fn json_for_html_script(value: &serde_json::Value) -> String {
    serde_json::to_string(value)
        .unwrap_or_default()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

async fn prettyunixtime(unix_time: i64) -> String {
    let dt: DateTime<Local> = DateTime::from_timestamp(unix_time, 0).unwrap().into();
    format!(
        "{}:{} {}/{} {}",
        dt.hour(),
        dt.minute(),
        dt.day(),
        dt.month(),
        dt.year()
    )
}

fn get_header_value(
    headers: &HeaderMap,
    header_name: axum::http::header::HeaderName,
) -> Option<String> {
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[derive(Serialize, Deserialize)]
struct CommonHeaders {
    accept_language: Option<String>,
}
fn extract_common_headers(headers: &HeaderMap) -> CommonHeaders {
    let accept_language = get_header_value(headers, ACCEPT_LANGUAGE);

    CommonHeaders { accept_language }
}

fn build_session_cookie(token: &str, config: &Config) -> String {
    let domain_part = match &config.custom_session_domain {
        Some(d) => format!("; Domain={}", d),
        None => String::new(),
    };
    let secure = if url::Url::parse(&config.site_url)
        .ok()
        .is_some_and(|url| url.scheme() == "https")
    {
        "; Secure"
    } else {
        ""
    };
    let max_age = config
        .session_ttl_seconds
        .unwrap_or(DEFAULT_SESSION_TTL_SECONDS);
    format!(
        "session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}{}",
        token, max_age, secure, domain_part
    )
}

fn clear_session_cookie(config: &Config) -> String {
    let domain_part = config
        .custom_session_domain
        .as_ref()
        .map(|domain| format!("; Domain={domain}"))
        .unwrap_or_default();
    let secure = if url::Url::parse(&config.site_url)
        .ok()
        .is_some_and(|url| url.scheme() == "https")
    {
        "; Secure"
    } else {
        ""
    };
    format!(
        "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}{}",
        secure, domain_part
    )
}

async fn create_authenticated_session(
    redis: &mut RedisConn,
    login: &str,
    config: &Config,
) -> Result<String, redis::RedisError> {
    let token = generate_secure_string();
    let ttl = config
        .session_ttl_seconds
        .unwrap_or(DEFAULT_SESSION_TTL_SECONDS);
    redis
        .set_ex::<_, _, ()>(format!("session:{token}"), login, ttl)
        .await?;
    Ok(token)
}

async fn rate_limit_exceeded(
    redis: &mut RedisConn,
    key: &str,
    limit: u64,
    window_seconds: i64,
) -> bool {
    let count: Result<u64, _> = redis.incr(key, 1u8).await;
    let Ok(count) = count else {
        return false;
    };

    if count == 1 {
        let _: Result<bool, _> = redis.expire(key, window_seconds).await;
    }
    count > limit
}

/// Parse cookies from ALL Cookie header entries (HTTP/2 may split them
/// into separate header fields instead of the single semicolon-delimited
/// header used in HTTP/1.1).
fn parse_all_cookies(headers: &HeaderMap) -> AHashMap<String, String> {
    let mut cookies = AHashMap::new();
    for value in headers.get_all("Cookie") {
        if let Ok(s) = value.to_str() {
            for cookie in s.split(';').map(|c| c.trim()) {
                let mut parts = cookie.splitn(2, '=');
                if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                    cookies.insert(key.to_string(), val.to_string());
                }
            }
        }
    }
    cookies
}

async fn get_user_login(
    headers: HeaderMap,
    db: &ScyllaDb,
    mut redis: RedisConn,
) -> Option<User> {
    let session_cookie = parse_all_cookies(&headers)
        .get("session")?
        .to_owned();

    let login: String = redis
        .get(format!("session:{}", session_cookie))
        .await
        .ok()?;

    let row = db.session.execute_unpaged(&db.get_user_by_login, (&login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>, Option<String>)>().ok().flatten())?;

    Some(User {
        login,
        name: row.0.unwrap_or_default(),
        profile_picture: row.1,
    })
}

async fn can_access_media_request(
    headers: &HeaderMap,
    db: &ScyllaDb,
    redis: RedisConn,
    medium_id: &str,
) -> bool {
    if !is_valid_resource_id(medium_id) {
        return false;
    }
    let row = db
        .session
        .execute_unpaged(&db.get_media_basic, (medium_id,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(
                String,
                String,
                String,
                String,
                Option<String>,
                String,
            )>()
            .ok()
            .flatten()
        });
    let Some((_id, _name, owner, visibility, restricted_to_group, _media_type)) = row else {
        return false;
    };
    if visibility == "public" {
        return true;
    }

    let user = get_user_login(headers.clone(), db, redis.clone()).await;
    can_access_restricted(
        db,
        &visibility,
        restricted_to_group.as_deref(),
        &owner,
        &user,
        redis,
    )
    .await
}

async fn is_public_profile_asset(db: &ScyllaDb, medium_id: &str, relative_path: &str) -> bool {
    let requested_file = relative_path
        .strip_prefix(medium_id)
        .and_then(|path| path.strip_prefix('/'))
        .unwrap_or_default();
    if !matches!(
        requested_file,
        "thumbnail-small.avif" | "thumbnail-sm.avif" | "thumbnail.avif" | "picture.avif"
    ) {
        return false;
    }

    let medium = db
        .session
        .execute_unpaged(&db.get_media_basic, (medium_id,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(
                String,
                String,
                String,
                String,
                Option<String>,
                String,
            )>()
            .ok()
            .flatten()
        });
    let Some((_id, _name, owner, visibility, _restricted_group, media_type)) = medium else {
        return false;
    };
    if visibility != "hidden" || media_type != "picture" {
        return false;
    }

    let profile_picture = db
        .session
        .execute_unpaged(&db.get_user_profile_picture, (&owner,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(Option<String>,)>()
                .ok()
                .flatten()
        })
        .and_then(|row| row.0);
    if profile_picture.as_deref() == Some(medium_id) {
        return true;
    }

    db.session
        .execute_unpaged(&db.get_user_channel_picture, (&owner,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(Option<String>,)>()
                .ok()
                .flatten()
        })
        .and_then(|row| row.0)
        .as_deref()
        == Some(medium_id)
}

async fn can_access_list_request(
    headers: &HeaderMap,
    db: &ScyllaDb,
    redis: RedisConn,
    list_id: &str,
) -> bool {
    let row = db
        .session
        .execute_unpaged(&db.get_list_by_id, (list_id,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(
                String,
                String,
                String,
                String,
                Option<String>,
                i64,
            )>()
            .ok()
            .flatten()
        });
    let Some((_id, _name, owner, visibility, restricted_to_group, _created)) = row else {
        return false;
    };

    let user = get_user_login(headers.clone(), db, redis.clone()).await;
    can_access_restricted(
        db,
        &visibility,
        restricted_to_group.as_deref(),
        &owner,
        &user,
        redis,
    )
    .await
}

async fn is_logged(user: Option<User>) -> bool {
    user.as_ref().is_some_and(|u| !u.login.is_empty())
}

fn format_file_size(size_bytes: usize) -> String {
    let size = size_bytes as f64;
    if size >= 1_000_000_000.0 {
        format!("{:.2} GB", size / 1_000_000_000.0)
    } else if size >= 1_000_000.0 {
        format!("{:.2} MB", size / 1_000_000.0)
    } else if size >= 1_000.0 {
        format!("{:.2} KB", size / 1_000.0)
    } else {
        format!("{} bytes", size_bytes)
    }
}

fn generate_medium_id() -> String {
    let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";

    (0..10)
        .map(|_| {
            let idx = rand::random_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

fn detect_medium_type_mime(mime: String) -> String {
    let mime_type = mime.to_ascii_lowercase();

    // --- Video ---
    if mime_type.contains("video")
        || matches!(
            mime_type.as_str(),
            "application/x-matroska"
                | "application/ogg"  // .ogv
        )
    {
        return "video".to_owned();
    }

    // --- Audio ---
    if mime_type.contains("audio")
        || matches!(
            mime_type.as_str(),
            "application/ogg"        // .ogg / .oga
                | "application/x-ogg"
                | "application/flac"
                | "application/x-flac"
                | "application/mp4"  // audio-only mp4
        )
    {
        return "audio".to_owned();
    }

    // --- Picture ---
    if mime_type.contains("image")
        || matches!(
            mime_type.as_str(),
            "application/dicom"      // medical imaging
        )
    {
        return "picture".to_owned();
    }

    // --- 3D Objects ---
    if mime_type.starts_with("model/")
        || matches!(
            mime_type.as_str(),
            "application/x-3ds"
                | "application/vnd.ms-pki.stl"
                | "application/octet-stream+glb"
        )
    {
        return "object_3d".to_owned();
    }

    // --- Document: PDF ---
    if mime_type == "application/pdf" {
        return "document_pdf".to_owned();
    }

    // --- Document: Writer (word processors) ---
    if matches!(
        mime_type.as_str(),
        // OpenDocument
        "application/vnd.oasis.opendocument.text"
            | "application/vnd.oasis.opendocument.text-template"
            // Legacy MS Word
            | "application/msword"
            // Modern MS Word
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
            // Apple Pages
            | "application/vnd.apple.pages"
            // Rich Text / plain text variants
            | "application/rtf"
            | "text/rtf"
    ) {
        return "document_writer".to_owned();
    }

    // --- Document: Spreadsheet ---
    if matches!(
        mime_type.as_str(),
        // OpenDocument
        "application/vnd.oasis.opendocument.spreadsheet"
            | "application/vnd.oasis.opendocument.spreadsheet-template"
            // Legacy MS Excel
            | "application/vnd.ms-excel"
            // Modern MS Excel
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
            // Apple Numbers
            | "application/vnd.apple.numbers"
    ) {
        return "document_spreadsheet".to_owned();
    }

    // --- Document: Presentation ---
    if matches!(
        mime_type.as_str(),
        // OpenDocument
        "application/vnd.oasis.opendocument.presentation"
            | "application/vnd.oasis.opendocument.presentation-template"
            // Legacy MS PowerPoint
            | "application/vnd.ms-powerpoint"
            // Modern MS PowerPoint
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.openxmlformats-officedocument.presentationml.template"
            | "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
            // Apple Keynote
            | "application/vnd.apple.keynote"
    ) {
        return "document_presentation".to_owned();
    }

    "other".to_owned()
}

async fn copy_dir(src: &str, dest: &str) -> io::Result<()> {
    let src_path = std::path::Path::new(src);
    let dest_path = std::path::Path::new(dest);

    if !dest_path.exists() {
        fs::create_dir_all(dest_path).await?;
    }

    let mut entries = fs::read_dir(src_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_entry_path = entry.path();
        let dest_entry_path = dest_path.join(entry.file_name());

        if src_entry_path.is_dir() {
            Box::pin(copy_dir(
                src_entry_path.to_str().unwrap(),
                dest_entry_path.to_str().unwrap(),
            ))
            .await?;
        } else {
            fs::copy(&src_entry_path, &dest_entry_path).await?;
        }
    }

    Ok(())
}

async fn move_dir(src: &str, dest: &str) -> io::Result<()> {
    copy_dir(src, dest).await?;
    fs::remove_dir_all(src).await?;
    Ok(())
}

const SYSTEM_GROUP_ALL_REGISTERED: &str = "__all_registered__";
const SYSTEM_GROUP_SUBSCRIBERS: &str = "__subscribers__";
const MAX_PAGE: i64 = 10_000;

fn valid_page(page: i64) -> bool {
    (0..=MAX_PAGE).contains(&page)
}

fn page_offset(page: usize, per_page: usize) -> Option<usize> {
    (page <= MAX_PAGE as usize)
        .then(|| page.checked_mul(per_page))
        .flatten()
}

fn is_system_group(group_id: &str) -> bool {
    group_id == SYSTEM_GROUP_ALL_REGISTERED || group_id == SYSTEM_GROUP_SUBSCRIBERS
}

async fn is_owned_or_system_group(db: &ScyllaDb, owner: &str, group_id: &str) -> bool {
    if is_system_group(group_id) {
        return true;
    }

    db.session
        .execute_unpaged(&db.get_group_by_id, (group_id,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(String, String, String)>()
                .ok()
                .flatten()
        })
        .is_some_and(|(_id, _name, group_owner)| group_owner == owner)
}

fn system_groups_for_owner(owner: &str) -> Vec<UserGroup> {
    vec![
        UserGroup {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: owner.to_owned(),
        },
        UserGroup {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: owner.to_owned(),
        },
    ]
}

async fn is_subscribed(db: &ScyllaDb, subscriber: &str, target: &str) -> bool {
    db.session.execute_unpaged(&db.is_subscribed, (subscriber, target))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.maybe_first_row::<(String,)>().ok().flatten().is_some())
        .unwrap_or(false)
}

async fn is_group_member(db: &ScyllaDb, group_id: &str, user_login: &str) -> bool {
    db.session
        .execute_unpaged(&db.is_group_member, (group_id, user_login))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .is_some()
}

async fn can_access_restricted(db: &ScyllaDb, visibility: &str, restricted_to_group: Option<&str>, owner: &str, user: &Option<User>, _redis: RedisConn) -> bool {
    if user.as_ref().is_some_and(|candidate| candidate.login == owner) {
        return true;
    }

    match visibility {
        "public" => true,
        "restricted" => {
            if let Some(u) = user {
                if let Some(group_id) = restricted_to_group {
                    if group_id == SYSTEM_GROUP_ALL_REGISTERED {
                        return true; // user is logged in
                    }
                    if group_id == SYSTEM_GROUP_SUBSCRIBERS {
                        return is_subscribed(db, &u.login, owner).await;
                    }
                    return is_group_member(db, group_id, &u.login).await;
                }
            }
            false
        }
        _ => false,
    }
}

/// Generate a timestamp-based comment ID (millisecond precision + random suffix)
fn generate_comment_id() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    // Shift left 16 bits, add random lower bits for uniqueness
    (now << 16) | (rand::random_range(0..65536) as i64)
}
