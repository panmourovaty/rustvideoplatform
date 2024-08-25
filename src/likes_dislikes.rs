async fn hx_like(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    let update_likes = sqlx::query!(
        "UPDATE media SET likes = likes + 1 WHERE id=$1 RETURNING likes;",
        mediumid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");
    Html(update_likes.likes.to_string())
}

async fn hx_dislike(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    let update_dislikes = sqlx::query!(
        "UPDATE media SET dislikes = dislikes + 1 WHERE id=$1 RETURNING dislikes;",
        mediumid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");
    Html(update_dislikes.dislikes.to_string())
}