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

    let mut rng = rng();
    let secure_string: String = (0..STRING_LEN)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    secure_string
}

fn parse_cookie_header(header: &str) -> AHashMap<String, String> {
    let mut cookies = AHashMap::new();
    for cookie in header.split(';').map(|s| s.trim()) {
        let mut parts = cookie.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            cookies.insert(key.to_string(), value.to_string());
        }
    }
    cookies
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
fn extract_common_headers(headers: &HeaderMap) -> Result<CommonHeaders, &'static str> {
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or("Missing or invalid 'Host' header")?;

    let user_agent = get_header_value(headers, USER_AGENT);
    let accept_language = get_header_value(headers, ACCEPT_LANGUAGE);
    let cookie = get_header_value(headers, COOKIE);

    Ok(CommonHeaders {
        host,
        user_agent,
        accept_language,
        cookie,
    })
}

async fn get_user_login(
    headers: HeaderMap,
    pool: &PgPool,
    mut redis: RedisConn,
) -> Option<User> {
    let session_cookie = parse_cookie_header(headers.get("Cookie")?.to_str().ok()?)
        .get("session")?
        .to_owned();

    let login: String = redis
        .get(format!("session:{}", session_cookie))
        .await
        .ok()?;

    let name = sqlx::query!("SELECT name FROM users WHERE login=$1;", login)
        .fetch_one(pool)
        .await
        .ok()?
        .name;

    Some(User {
        login,
        name,
    })
}

async fn is_logged(user: Option<User>) -> bool {
    let isloggedin: bool;
    if user.is_some() && user.unwrap().login != "".to_owned() {
        isloggedin = true;
    }
    else {
        isloggedin = false;
    }
    isloggedin
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
    let mut rng = rand::rng();
    let random_string: String = (0..10)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect();
    random_string
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

async fn get_user_group_ids(pool: &PgPool, user_login: &str, mut redis: RedisConn) -> Vec<String> {
    if user_login.is_empty() {
        return vec![];
    }

    let redis_key = format!("user:{}:groups", user_login);

    // Try cache first
    let cached: Result<Vec<String>, _> = redis.smembers(&redis_key).await;
    if let Ok(groups) = cached {
        if !groups.is_empty() {
            return groups;
        }
    }

    // Cache miss - load from DB
    let groups: Vec<String> = sqlx::query_scalar("SELECT group_id FROM user_group_members WHERE user_login = $1")
        .bind(user_login)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    if !groups.is_empty() {
        let _: Result<(), _> = redis.sadd(&redis_key, &groups).await;
        let _: Result<(), _> = redis.expire(&redis_key, 3600).await;
    }

    groups
}

async fn is_group_member(pool: &PgPool, group_id: &str, user_login: &str, redis: RedisConn) -> bool {
    let groups = get_user_group_ids(pool, user_login, redis).await;
    groups.contains(&group_id.to_owned())
}

async fn can_access_restricted(pool: &PgPool, visibility: &str, restricted_to_group: Option<&str>, owner: &str, user: &Option<User>, redis: RedisConn) -> bool {
    match visibility {
        "public" => true,
        "restricted" => {
            if let Some(u) = user {
                if u.login == owner {
                    return true;
                }
                if let Some(group_id) = restricted_to_group {
                    return is_group_member(pool, group_id, &u.login, redis).await;
                }
            }
            false
        }
        _ => true // "hidden" - accessible via direct link (existing behavior)
    }
}
