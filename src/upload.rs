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
) -> Html<String> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }

    let upload_dir = std::path::Path::new("upload");
    let start_time = tokio::time::Instant::now();

    // Process each file in the multipart form
    let mut response_html = String::new();
    response_html.push_str("<h2>File uploaded successfully!</h2>");
    response_html.push_str("<table border=\"1\" cellpadding=\"10\">");
    response_html.push_str("<tr><th>File Name</th><th>File Size (MB)</th><th>File Type</th><th>Upload Duration (Seconds)</th></tr>");

    while let Some(field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap().to_string();
        let file_type = field.content_type().map(|ct| ct.to_string()).unwrap_or_else(|| "unknown".to_string());
        let data = field.bytes().await.unwrap();
        let file_size_mb = data.len() as f64 / (1024.0 * 1024.0);

        let file_path = upload_dir.join(&file_name);

        // Save the file to the "upload" directory
        let mut file = tokio::fs::File::create(file_path).await.unwrap();
        file.write_all(&data).await.unwrap();

        // Calculate the upload duration
        let duration = start_time.elapsed().as_secs_f64();

        // Add the file details to the response
        response_html.push_str(&format!(
            "<tr><td>{}</td><td>{:.2}</td><td>{}</td><td>{:.2}</td></tr>",
            file_name, file_size_mb, file_type, duration
        ));
    }

    response_html.push_str("</table>");

    Html(response_html)
}
