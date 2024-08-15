#[derive(Template)]
#[template(path = "pages/hx-reccomended.html", escape = "none")]
struct HXReccomendedTemplate {
    recommendations: Vec<Medium>,
}
async fn hx_recommended(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let recommendations: Vec<Medium> = sqlx::query_as!(
        Medium,
        "SELECT id, name, owner, views, type FROM media WHERE public = true AND id != $1 LIMIT 20;",
        mediumid
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to fetch recommendations".into())
            .unwrap()
    })?;

    let template = HXReccomendedTemplate { recommendations };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}