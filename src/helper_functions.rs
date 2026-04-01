fn minifi_html(html: String) -> Vec<u8> {
    let cfg = minify_html_onepass::Cfg {
        minify_css: true,
        minify_js: true,
        ..Default::default()
    };

    minify_html_onepass::copy(html.as_bytes(), &cfg).unwrap()
}

fn read_lines_to_vec(filepath: &str) -> Vec<String> {
    let file = std::fs::File::open(filepath).unwrap();
    let reader = std::io::BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
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
    host: String,
    user_agent: Option<String>,
    accept_language: Option<String>,
    cookie: Option<String>,
}
fn extract_common_headers(headers: &HeaderMap) -> CommonHeaders {
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost")
        .to_string();

    let user_agent = get_header_value(headers, USER_AGENT);
    let accept_language = get_header_value(headers, ACCEPT_LANGUAGE);
    let cookie = get_header_value(headers, COOKIE);

    CommonHeaders {
        host,
        user_agent,
        accept_language,
        cookie,
    }
}

fn build_session_cookie(token: &str, config: &Config) -> String {
    let domain_part = match &config.custom_session_domain {
        Some(d) => format!("; Domain={}", d),
        None => String::new(),
    };
    format!(
        "session={}; Path=/; HttpOnly; SameSite=Lax{}",
        token, domain_part
    )
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

fn is_system_group(group_id: &str) -> bool {
    group_id == SYSTEM_GROUP_ALL_REGISTERED || group_id == SYSTEM_GROUP_SUBSCRIBERS
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

async fn is_group_member(db: &ScyllaDb, group_id: &str, user_login: &str, mut redis: RedisConn) -> bool {
    let redis_key = format!("group:{}:members", group_id);

    // Check if membership set is cached in Redis
    let key_exists: bool = redis.exists(&redis_key).await.unwrap_or(false);

    if key_exists {
        return redis.sismember(&redis_key, user_login).await.unwrap_or(false);
    }

    // Cache miss - load all members from DB and cache in Redis
    let members: Vec<String> = db.session.execute_unpaged(&db.get_group_members, (group_id,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| {
            rows.rows::<(String,)>()
                .unwrap()
                .filter_map(|r| r.ok())
                .map(|r| r.0)
                .collect()
        })
        .unwrap_or_default();

    let is_member = members.contains(&user_login.to_owned());

    if !members.is_empty() {
        let _: Result<(), _> = redis.sadd(&redis_key, &members).await;
        let _: Result<(), _> = redis.expire(&redis_key, 3600).await;
    }

    is_member
}

async fn can_access_restricted(db: &ScyllaDb, visibility: &str, restricted_to_group: Option<&str>, owner: &str, user: &Option<User>, redis: RedisConn) -> bool {
    match visibility {
        "public" => true,
        "restricted" => {
            if let Some(u) = user {
                if u.login == owner {
                    return true;
                }
                if let Some(group_id) = restricted_to_group {
                    if group_id == SYSTEM_GROUP_ALL_REGISTERED {
                        return true; // user is logged in
                    }
                    if group_id == SYSTEM_GROUP_SUBSCRIBERS {
                        return is_subscribed(db, &u.login, owner).await;
                    }
                    return is_group_member(db, group_id, &u.login, redis).await;
                }
            }
            false
        }
        _ => false // "hidden" and unknown - not shown in feeds, only accessible via direct link
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
