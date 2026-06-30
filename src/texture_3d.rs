#[derive(Template)]
#[template(path = "pages/hx-studio-edit-textures.html")]
struct HXStudioEditTexturesTemplate {
    medium_id: String,
    locale: RequestLocale,
}

async fn hx_studio_edit_textures_tab(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    if !is_valid_resource_id(&mediumid) {
        return Html(Vec::new());
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    let owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(o,)| o);

    if owner.as_deref() != Some(user_info.login.as_str()) {
        return Html(minifi_html("".to_owned()));
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&user_info), &common_headers, &db, &localization, &config).await;
    let template = HXStudioEditTexturesTemplate { medium_id: mediumid, locale };
    Html(minifi_html(template.render().unwrap()))
}

async fn studio_textures_list(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    if !is_valid_resource_id(&mediumid) {
        return Json(serde_json::json!({ "textures": [] }));
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::json!({ "textures": [] }));
    }
    let user_info = user_info.unwrap();

    let owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(o,)| o);

    if owner.as_deref() != Some(user_info.login.as_str()) {
        return Json(serde_json::json!({ "textures": [] }));
    }

    let textures_dir = format!("source/{}/textures", mediumid);
    let mut textures: Vec<String> = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&textures_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let ext = std::path::Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga" | "ktx2") {
                textures.push(name);
            }
        }
    }

    textures.sort();
    Json(serde_json::json!({ "textures": textures }))
}

async fn studio_textures_upload(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    mut multipart: Multipart,
) -> Response<Body> {
    if !is_valid_resource_id(&mediumid) {
        return json_resp(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid media id"}),
        );
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(o,)| o);

    match owner {
        Some(ref o) if o == &user_info.login => {}
        Some(_) => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let mut file_bytes: Vec<u8> = Vec::new();
    let mut original_filename = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            original_filename = field
                .file_name()
                .unwrap_or("texture")
                .to_string();
            file_bytes = field.bytes().await.unwrap_or_default().to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"file is required\"}"))
            .unwrap();
    }

    // Validate extension
    let ext = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga" | "ktx2") {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"unsupported file type\"}"))
            .unwrap();
    }

    // Sanitise filename: keep only alphanumeric, dash, underscore, dot
    let safe_name: String = original_filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    let safe_name = if safe_name.is_empty() { format!("texture.{}", ext) } else { safe_name };

    let textures_dir = format!("source/{}/textures", mediumid);
    if tokio::fs::create_dir_all(&textures_dir).await.is_err() {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to create textures directory\"}"))
            .unwrap();
    }

    let dest = format!("{}/{}", textures_dir, safe_name);
    if tokio::fs::write(&dest, &file_bytes).await.is_err() {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to write texture file\"}"))
            .unwrap();
    }

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}

#[derive(Deserialize)]
struct DeleteTextureForm {
    filename: String,
}

async fn studio_textures_delete(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Json(body): Json<DeleteTextureForm>,
) -> Response<Body> {
    if !is_valid_resource_id(&mediumid) {
        return json_resp(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid media id"}),
        );
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(o,)| o);

    match owner {
        Some(ref o) if o == &user_info.login => {}
        Some(_) => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    // Validate filename: reject any path traversal
    let filename = &body.filename;
    if filename.contains('/') || filename.contains('\\') || filename.starts_with('.') {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"invalid filename\"}"))
            .unwrap();
    }

    let path = format!("source/{}/textures/{}", mediumid, filename);
    let _ = tokio::fs::remove_file(&path).await;

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}

async fn studio_textures_apply(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    if !is_valid_resource_id(&mediumid) {
        return json_resp(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid media id"}),
        );
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|(o,)| o);

    match owner {
        Some(ref o) if o == &user_info.login => {}
        Some(_) => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let glb_path = format!("source/{}/model.glb", mediumid);
    if !std::path::Path::new(&glb_path).exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"model.glb not found — model may still be processing\"}"))
            .unwrap();
    }

    let textures_dir = format!("source/{}/textures", mediumid);
    if !std::path::Path::new(&textures_dir).exists() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"no textures uploaded yet\"}"))
            .unwrap();
    }

    // Enqueue the retexture job for the processor — the processor owns all Blender work
    let enqueue_result = db.session.execute_unpaged(
        &db.insert_unprocessed_concept,
        (&mediumid, "object_3d_retexture"),
    ).await;

    match enqueue_result {
        Ok(_) => Response::builder()
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"ok\":true,\"queued\":true}"))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to queue processing job\"}"))
            .unwrap(),
    }
}
