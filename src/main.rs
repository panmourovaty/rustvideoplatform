#![forbid(unsafe_code)]
use std::fs;

use argon2::Argon2;
use argon2::PasswordVerifier;
use axum::http::StatusCode;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use ahash::AHashMap;
use argon2::password_hash::{rand_core::OsRng, PasswordHash};
use askama::Template;
use axum::{
    extract::Form,
    extract::Path,
    http::header::HeaderMap,
    http::header::{ACCEPT_LANGUAGE, COOKIE, HOST, USER_AGENT},
    response::Html,
    response::IntoResponse,
    routing::get,
    routing::post,
    Extension, Router,
};
use chrono::{DateTime, Datelike, Local, Timelike};
use memory_serve::{load_assets, MemoryServe};
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize, Clone)]
struct Config {
    dbconnection: String,
    instancename: String,
    welcome: String,
}
#[tokio::main]
async fn main() {
    let config: Config = serde_json::from_str(&fs::read_to_string("config.json").unwrap()).unwrap();

    let pool = PgPoolOptions::new()
        .max_connections(100)
        .connect(&config.dbconnection)
        .await
        .unwrap();

    let memory_router = MemoryServe::new(load_assets!("assets/static")).into_router();

    let session_store: Arc<Mutex<AHashMap<String, String>>> =
        Arc::new(Mutex::new(AHashMap::default()));

    let app = Router::new()
        .route("/", get(home))
        .route("/login", get(login))
        .route("/trending", get(trending))
        .route("/hx/trending", get(hx_trending))
        .route("/video/:videoid", get(video))
        .route("/hx/comments/:videoid", get(hx_comments))
        .route("/hx/reccomended/:videoid", get(hx_reccomended))
        .route("/hx/new_view/:videoid", get(hx_new_view))
        .route("/hx/like/:videoid", get(hx_like))
        .route("/hx/dislike/:videoid", get(hx_dislike))
        .route("/hx/login", post(hx_login))
        .route("/hx/logout", get(hx_logout))
        .route("/hx/usernav", get(hx_usernav))
        .route("/hx/sidebar/:active_item", get(hx_sidebar))
        .route("/hx/searchsuggestions", post(hx_search_suggestions))
        .nest("/source", axum_static::static_router("source"))
        .layer(Extension(pool))
        .layer(Extension(config))
        .layer(Extension(session_store))
        .merge(memory_router);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn minifi_html(html: String) -> Vec<u8> {
    let cfg = minify_html_onepass::Cfg {
        minify_css: true,
        minify_js: true,
        ..Default::default()
    };

    minify_html_onepass::copy(html.as_bytes(), &cfg).unwrap()
}

#[derive(Template)]
#[template(path = "pages/component-sidebar.html", escape = "none")]
struct SidebarComponentTemplate {
    config: Config,
    active_item: String,
}
fn generate_sidebar(config: &Config, active_item: String) -> String {
    let template = SidebarComponentTemplate {
        config: config.to_owned(),
        active_item,
    };
    template.render().unwrap()
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
fn get_header_value(
    headers: &HeaderMap,
    header_name: axum::http::header::HeaderName,
) -> Option<String> {
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[derive(Template)]
#[template(path = "pages/video.html", escape = "none")]
struct VideoTemplate {
    sidebar: String,
    video_id: String,
    video_name: String,
    video_description: String,
    video_owner: String,
    video_likes: i64,
    video_dislikes: i64,
    video_upload: String,
    video_views: i64,
    config: Config,
    common_headers: CommonHeaders,
}
async fn video(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Path(videoid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers).unwrap();
    let video = sqlx::query!(
        "SELECT id,name,description,upload,owner,likes,dislikes,views FROM videos WHERE id=$1;",
        videoid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");

    let sidebar = generate_sidebar(&config, "video".to_owned());
    let template = VideoTemplate {
        sidebar,
        video_id: video.id,
        video_name: video.name,
        video_description: video.description,
        video_owner: video.owner,
        video_likes: video.likes,
        video_dislikes: video.dislikes,
        video_upload: prettyunixtime(video.upload).await,
        video_views: video.views,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct Comment {
    id: i64,
    user: String,
    text: String,
    time: i64,
}
#[derive(Template)]
#[template(path = "pages/hx-comments.html", escape = "none")]
struct HXCommentsTemplate {
    comments: Vec<Comment>,
}
async fn hx_comments(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let comments_records = sqlx::query!(
        "SELECT id,user,text,time FROM comments WHERE video=$1;",
        videoid
    )
    .fetch_all(&pool)
    .await
    .expect("Nemohu provést dotaz");

    let mut comments: Vec<Comment> = Vec::new();
    for record in comments_records {
        let new_comment: Comment = Comment {
            id: record.id,
            user: record.user.unwrap(),
            text: record.text,
            time: record.time,
        };
        comments.push(new_comment);
    }
    let template = HXCommentsTemplate { comments };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct Video {
    id: String,
    name: String,
    owner: String,
    views: i64,
}
#[derive(Template)]
#[template(path = "pages/hx-reccomended.html", escape = "none")]
struct HXReccomendedTemplate {
    reccomendations: Vec<Video>,
}
async fn hx_reccomended(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let comments_records = sqlx::query!(
        "SELECT id,name,owner,views FROM videos WHERE public=true ORDER BY random() LIMIT 20;"
    )
    .fetch_all(&pool)
    .await
    .expect("Nemohu provést dotaz");

    let mut reccomendations: Vec<Video> = Vec::new();
    for record in comments_records {
        if record.id != videoid {
            let new_reccomendation: Video = Video {
                id: record.id,
                name: record.name,
                owner: record.owner,
                views: record.views,
            };
            reccomendations.push(new_reccomendation);
        }
    }
    let template = HXReccomendedTemplate { reccomendations };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_new_view(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<String> {
    let update_views = sqlx::query!(
        "UPDATE videos SET views = views + 1 WHERE id=$1 RETURNING views;",
        videoid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");
    Html(update_views.views.to_string())
}

async fn hx_like(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<String> {
    let update_likes = sqlx::query!(
        "UPDATE videos SET likes = likes + 1 WHERE id=$1 RETURNING likes;",
        videoid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");
    Html(update_likes.likes.to_string())
}
async fn hx_dislike(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<String> {
    let update_dislikes = sqlx::query!(
        "UPDATE videos SET dislikes = dislikes + 1 WHERE id=$1 RETURNING dislikes;",
        videoid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");
    Html(update_dislikes.dislikes.to_string())
}

#[derive(Serialize, Deserialize)]
struct LoginForm {
    login: String,
    password: String,
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
struct User {
    login: String,
    name: String,
}
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
    let try_user = get_user_login(headers, pool, session_store).await;
    if try_user.is_some() {
        let user = try_user.unwrap();
        let template = HXUsernavTemplate { user };
        return Html(minifi_html(template.render().unwrap()));
    } else {
        let result = format!("<a href=\"/login\"><button class=\"btn text-white\"><i class=\"fa-solid fa-user mx-2\"></i>Log in</button></a>");
        return Html(minifi_html(result));
    }
}

async fn get_user_login(
    headers: HeaderMap,
    pool: PgPool,
    session_store: Arc<Mutex<AHashMap<String, String>>>,
) -> Option<User> {
    let session_cookie = parse_cookie_header(headers.get("Cookie")?.to_str().ok()?)
        .get("session")?
        .to_owned();

    let session_store_guard = session_store.lock().await;
    let login = session_store_guard.get(&session_cookie)?;

    let name = sqlx::query!("SELECT name FROM users WHERE login=$1;", login)
        .fetch_one(&pool)
        .await
        .ok()?
        .name;

    Some(User {
        login: login.to_owned(),
        name,
    })
}

#[derive(Template)]
#[template(path = "pages/trending.html", escape = "none")]
struct TrendingTemplate {
    sidebar: String,
}
async fn trending(Extension(config): Extension<Config>) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "trending".to_owned());
    let template = TrendingTemplate { sidebar };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-trending.html", escape = "none")]
struct HXTrendingTemplate {
    reccomendations: Vec<Video>,
}
async fn hx_trending(Extension(pool): Extension<PgPool>) -> axum::response::Html<Vec<u8>> {
    let comments_records = sqlx::query!(
        "SELECT id,name,owner,views FROM videos WHERE public=true ORDER BY likes DESC LIMIT 100;"
    )
    .fetch_all(&pool)
    .await
    .expect("Nemohu provést dotaz");

    let mut reccomendations: Vec<Video> = Vec::new();
    for record in comments_records {
        let new_reccomendation: Video = Video {
            id: record.id,
            name: record.name,
            owner: record.owner,
            views: record.views,
        };
        reccomendations.push(new_reccomendation);
    }
    let template = HXTrendingTemplate { reccomendations };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-sidebar.html", escape = "none")]
struct HXSidebarTemplate {
    active_item: String,
    login: String,
}
async fn hx_sidebar(
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Extension(pool): Extension<PgPool>,
    Path(active_item): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, pool, session_store).await;
    if user.is_some() {
        let template = HXSidebarTemplate {
            active_item,
            login: user.unwrap().login,
        };
        Html(minifi_html(template.render().unwrap()))
    } else {
        Html("".as_bytes().to_vec())
    }
}

#[derive(Template)]
#[template(path = "pages/home.html", escape = "none")]
struct HomeTemplate {
    sidebar: String,
    config: Config,
}
async fn home(Extension(config): Extension<Config>) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "home".to_owned());
    let template = HomeTemplate { config, sidebar };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct HXSearch {
    search: String,
}
#[derive(Template)]
#[template(path = "pages/hx-searchsuggestion.html", escape = "none")]
struct HXSearchSuggestions {
    suggestions: Vec<Video>,
}
async fn hx_search_suggestions(
    Extension(pool): Extension<PgPool>,
    Form(form): Form<HXSearch>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let search_term = format!("%{}%", form.search);
    let suggestions = sqlx::query_as!(
        Video,
        "SELECT id, name, owner, views FROM videos WHERE name ILIKE $1 LIMIT 10;",
        search_term
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| vec![]);

    if suggestions.is_empty() {
        return Html("<li><b>Nothing found</b></li>".to_owned());
    }

    let template = HXSearchSuggestions { suggestions };
    Html(template.render().unwrap())
}
