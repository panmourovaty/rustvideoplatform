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
    // Define the character set: a-z and 0-9
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const STRING_LEN: usize = 100;

    let mut rng = OsRng; // Use OsRng for cryptographically secure random number generation
    let secure_string: String = (0..STRING_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
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
    session_store: Arc<Mutex<AHashMap<String, String>>>,
) -> Option<User> {
    let session_cookie = parse_cookie_header(headers.get("Cookie")?.to_str().ok()?)
        .get("session")?
        .to_owned();

    let session_store_guard = session_store.lock().await;
    let login = session_store_guard.get(&session_cookie)?;

    let name = sqlx::query!("SELECT name FROM users WHERE login=$1;", login)
        .fetch_one(pool)
        .await
        .ok()?
        .name;

    Some(User {
        login: login.to_owned(),
        name,
    })
}