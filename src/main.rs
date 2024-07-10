#![forbid(unsafe_code)]
use std::fs;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use askama::Template;
use axum::{
    extract::Form, extract::Path, response::Html, routing::get, routing::post,
    Extension, Router,
};
use memory_serve::{load_assets, MemoryServe};
use serde::Deserialize;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use chrono::{DateTime, Local, Timelike, Datelike};

#[derive(Deserialize, Clone)]
struct Config {
    dbconnection: String,
    instancename: String,
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

    let app = Router::new()
        .route("/video/:videoid", get(video))
        .route("/hx/comments/:videoid", get(hx_comments))
        .route("/hx/reccomended/:videoid", get(hx_reccomended))
        .route("/hx/new_view/:videoid", get(hx_new_view))
        .route("/hx/like/:videoid", get(hx_like))
        .route("/hx/dislike/:videoid", get(hx_dislike))
        .nest("/source", axum_static::static_router("source"))
        .layer(Extension(pool))
        .layer(Extension(config))
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
fn generate_sidebar(config: Config, active_item: String) -> String {
    let template = SidebarComponentTemplate {
        config,
        active_item,
    };
    template.render().unwrap()
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
    video_views: i64
}
async fn video(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let video = sqlx::query!(
        "SELECT id,name,description,upload,owner,likes,dislikes,views FROM videos WHERE id=$1;",
        videoid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");

    let sidebar = generate_sidebar(config, "".to_owned());
    let template = VideoTemplate {
        sidebar,
        video_id: video.id,
        video_name: video.name,
        video_description: video.description,
        video_owner: video.owner,
        video_likes: video.likes,
        video_dislikes: video.dislikes,
        video_upload: prettyunixtime(video.upload).await,
        video_views: video.views
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
    let comments_records =
        sqlx::query!("SELECT id,name,owner FROM videos WHERE public=true ORDER BY random() LIMIT 20;")
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
            };
            reccomendations.push(new_reccomendation);
        }
    }
    let template = HXReccomendedTemplate { reccomendations };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_new_view(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>
) -> axum::response::Html<String> {
    let update_views =
        sqlx::query!("UPDATE videos SET views = views + 1 WHERE id=$1 RETURNING views;",videoid)
            .fetch_one(&pool)
            .await
            .expect("Nemohu provést dotaz");
    Html(update_views.views.to_string())
}

async fn hx_like(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>
) -> axum::response::Html<String> {
    let update_likes =
        sqlx::query!("UPDATE videos SET likes = likes + 1 WHERE id=$1 RETURNING likes;",videoid)
            .fetch_one(&pool)
            .await
            .expect("Nemohu provést dotaz");
    Html(update_likes.likes.to_string())
}
async fn hx_dislike(
    Extension(pool): Extension<PgPool>,
    Path(videoid): Path<String>
) -> axum::response::Html<String> {
    let update_dislikes =
        sqlx::query!("UPDATE videos SET dislikes = dislikes + 1 WHERE id=$1 RETURNING dislikes;",videoid)
            .fetch_one(&pool)
            .await
            .expect("Nemohu provést dotaz");
    Html(update_dislikes.dislikes.to_string())
}