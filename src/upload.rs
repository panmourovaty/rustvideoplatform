#[derive(Template)]
#[template(path = "pages/hx-studio-upload.html")]
struct HXStudioUploadTemplate {
    locale: RequestLocale,
}
async fn hx_studio_upload(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXStudioUploadTemplate { locale };
    Html(minifi_html(template.render().unwrap()))
}

async fn upload(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let current_user = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(current_user.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "studio".to_owned(),
        current_user.clone(),
        locale.clone(),
    );
    let template = StudioTemplate {
        sidebar,
        config,
        current_user,
        active_tab: "upload".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_upload(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Step 1: Authenticate User
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Ok(Html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    // Step 2: Setup directories
    let upload_dir = std::path::Path::new("upload");
    if tokio::fs::create_dir_all(upload_dir).await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(
                "<div class=\"alert alert-danger\">Failed to create upload directory</div>"
                    .to_owned(),
            ),
        ));
    }
    let medium_id = generate_medium_id();

    // Step 3: Process each field in the multipart form
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Html("<div class=\"alert alert-danger\">No file was provided</div>".to_owned()),
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Html(
                    "<div class=\"alert alert-danger\">Invalid upload request</div>".to_owned(),
                ),
            ));
        }
    };

    let file_name = field.file_name().unwrap_or("unnamed").to_string();
    let file_type = field
        .content_type()
        .map(|ct| ct.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Step 4: Open the file for writing
    let file_path = upload_dir.join(&medium_id);
    let mut file = match tokio::fs::File::create(&file_path).await {
        Ok(f) => f,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(
                    "<div class=\"alert alert-danger\">Failed to create upload file</div>"
                        .to_owned(),
                ),
            ));
        }
    };

    // Step 5: Stream and write chunks to the file
    let mut file_size: usize = 0;
    let mut field = field;
    loop {
        match field.chunk().await {
            Ok(Some(chunk)) => {
                file_size += chunk.len();
                if file.write_all(&chunk).await.is_err() {
                    // Clean up partial file on write error
                    let _ = tokio::fs::remove_file(&file_path).await;
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(
                            "<div class=\"alert alert-danger\">Failed to write upload</div>"
                                .to_owned(),
                        ),
                    ));
                }
            }
            Ok(None) => break,
            Err(_) => {
                // Clean up partial file on read error
                let _ = tokio::fs::remove_file(&file_path).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    Html(
                        "<div class=\"alert alert-danger\">Upload interrupted</div>".to_owned(),
                    ),
                ));
            }
        }
    }

    if file.flush().await.is_err() {
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(
                "<div class=\"alert alert-danger\">Failed to finalize file</div>".to_owned(),
            ),
        ));
    }

    // Step 6: Save metadata to the database
    let mut medium_type = detect_medium_type_mime(file_type.clone());
    // For generic MIME types, also check file extension for 3D model formats
    if medium_type == "other" || file_type.contains("octet-stream") {
        let ext = std::path::Path::new(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "glb" | "gltf" | "obj" | "fbx" | "stl" | "ply" | "dae"
            | "usd" | "usda" | "usdc" | "usdz" | "3ds" | "blend" => {
                medium_type = "object_3d".to_owned();
            }
            _ => {}
        }
    }
    let owner = user_info.unwrap().login;

    let insert_result = db.session.execute_unpaged(&db.insert_concept, (&medium_id, &file_name, &owner, &medium_type)).await;
    if insert_result.is_err() {
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<div class=\"alert alert-danger\">Failed to save upload</div>".to_owned()),
        ));
    }

    let owner_insert = db
        .session
        .execute_unpaged(
            &db.insert_concept_by_owner,
            (&owner, &medium_id, &file_name, &medium_type),
        )
        .await;
    let queue_insert = if owner_insert.is_ok() {
        db.session
            .execute_unpaged(&db.insert_unprocessed_concept, (&medium_id, &medium_type))
            .await
    } else {
        let _ = db
            .session
            .execute_unpaged(&db.delete_concept, (&medium_id,))
            .await;
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<div class=\"alert alert-danger\">Failed to save upload</div>".to_owned()),
        ));
    };
    if queue_insert.is_err() {
        let _ = db
            .session
            .execute_unpaged(&db.delete_concept, (&medium_id,))
            .await;
        let _ = db
            .session
            .execute_unpaged(&db.delete_concept_by_owner, (&owner, &medium_id))
            .await;
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<div class=\"alert alert-danger\">Failed to queue upload</div>".to_owned()),
        ));
    }

    // Step 7: Format success response
    let formatted_file_size = format_file_size(file_size);
    let response_html = format!(
        r#"<div class="text-center">
            <h3 class="text-success mb-3"><i class="fa-solid fa-circle-check"></i> Upload complete!</h3>
            <table class="table table-sm" style="max-width:400px;margin:0 auto">
                <tr><th>File</th><td>{}</td></tr>
                <tr><th>Size</th><td>{}</td></tr>
                <tr><th>Type</th><td>{}</td></tr>
            </table>
            <a href="/studio/concepts" class="btn btn-primary mt-3" preload="mouseover">
                <i class="fa-solid fa-arrow-right"></i> View Concepts
            </a>
        </div>"#,
        escape_html(&file_name), formatted_file_size, escape_html(&medium_type)
    );

    Ok(Html(response_html))
}
