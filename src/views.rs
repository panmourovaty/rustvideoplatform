async fn hx_new_view(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    let update_views = sqlx::query!(
        "UPDATE media SET views = views + 1 WHERE id=$1 RETURNING views;",
        mediumid
    )
    .fetch_one(&pool)
    .await
    .expect("Nemohu provést dotaz");
    Html(update_views.views.to_string())
}
