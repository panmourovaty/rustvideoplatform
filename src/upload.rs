#[derive(Template)]
#[template(path = "pages/upload.html", escape = "none")]
struct UploadTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn upload(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "upload".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = UploadTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_upload(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Html<String>, StatusCode> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Ok(Html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let mut response_message = String::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap_or("file").to_string();
        let content_type = field.content_type().unwrap().to_string();

        let file_id = Uuid::new_v4().to_string();
        let file_path = format!("uploads/{}-{}", file_id, file_name);

        let mut file = File::create(&file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let data = field
            .bytes()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        file.write_all(&data)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        response_message = format!(
            "<h1>File uploaded successfully!</h1><p>File name: {}</p><p>Content type: {}</p><p>Saved as: {}</p>",
            file_name, content_type, file_path
        );
    }

    Ok(Html(response_message))
}
