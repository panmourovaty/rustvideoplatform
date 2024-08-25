#[derive(Serialize, Deserialize)]
struct Comment {
    id: i64,
    user: Option<String>,
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
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let comments = sqlx::query_as!(
        Comment,
        "SELECT id,user,text,time FROM comments WHERE media=$1;",
        mediumid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXCommentsTemplate { comments };
    Html(minifi_html(template.render().unwrap()))
}