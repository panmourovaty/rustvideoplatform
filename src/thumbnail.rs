async fn studio_thumbnail_get(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    if !is_valid_resource_id(&mediumid) {
        return Json(serde_json::json!({ "exists": false }));
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::json!({ "exists": false }));
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Json(serde_json::json!({ "exists": false })),
    }

    let avif_path = format!("source/{}/thumbnail.avif", mediumid);
    let exists = std::path::Path::new(&avif_path).exists();
    Json(serde_json::json!({ "exists": exists }))
}

async fn studio_thumbnail_upload(
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

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
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

    let mut image_content = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            image_content = field.bytes().await.unwrap_or_default().to_vec();
            break;
        }
    }

    if image_content.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"image file is required\"}"))
            .unwrap();
    }

    // Write the uploaded image to a temp file
    let temp_path = format!("source/{}/.tmp_thumbnail_upload", mediumid);
    let source_dir = format!("source/{}", mediumid);
    let _ = tokio::fs::create_dir_all(&source_dir).await;

    if tokio::fs::write(&temp_path, &image_content).await.is_err() {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to write temp file\"}"))
            .unwrap();
    }

    // Convert to thumbnail.jpg, thumbnail.avif at 1280x720, and thumbnail-sm.avif at 352x198 using FFmpeg
    let avif_path = format!("source/{}/thumbnail.avif", mediumid);
    let jpg_path = format!("source/{}/thumbnail.jpg", mediumid);
    let sm_avif_path = format!("source/{}/thumbnail-sm.avif", mediumid);
    let temp_path_clone = temp_path.clone();
    let avif_path_clone = avif_path.clone();
    let jpg_path_clone = jpg_path.clone();
    let sm_avif_path_clone = sm_avif_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        use std::process::Command;

        let jpg_status = Command::new("ffmpeg")
            .args([
                "-nostdin", "-y",
                "-i", &temp_path_clone,
                "-vf", "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:black",
                "-frames:v", "1",
                "-update", "1",
                &jpg_path_clone,
            ])
            .status();

        let avif_status = Command::new("ffmpeg")
            .args([
                "-nostdin", "-y",
                "-i", &temp_path_clone,
                "-vf", "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:black,format=yuv420p",
                "-frames:v", "1",
                "-c:v", "libsvtav1",
                "-svtav1-params", "avif=1",
                "-crf", "28",
                "-update", "1",
                &avif_path_clone,
            ])
            .status();

        let sm_avif_status = Command::new("ffmpeg")
            .args([
                "-nostdin", "-y",
                "-i", &temp_path_clone,
                "-vf", "scale=352:198:force_original_aspect_ratio=decrease,pad=352:198:(ow-iw)/2:(oh-ih)/2:black,format=yuv420p",
                "-frames:v", "1",
                "-c:v", "libsvtav1",
                "-svtav1-params", "avif=1",
                "-crf", "28",
                "-update", "1",
                &sm_avif_path_clone,
            ])
            .status();

        let jpg_ok = jpg_status.map(|s| s.success()).unwrap_or(false);
        let avif_ok = avif_status.map(|s| s.success()).unwrap_or(false);
        let sm_avif_ok = sm_avif_status.map(|s| s.success()).unwrap_or(false);
        (jpg_ok, avif_ok, sm_avif_ok)
    }).await;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    match result {
        Ok((true, true, _)) => {
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"ok\":true}"))
                .unwrap()
        }
        Ok((jpg_ok, avif_ok, sm_avif_ok)) => {
            let msg = if !jpg_ok && !avif_ok {
                "{\"error\":\"thumbnail conversion failed\"}"
            } else if !jpg_ok {
                "{\"error\":\"JPG thumbnail conversion failed\"}"
            } else if !avif_ok {
                "{\"error\":\"AVIF thumbnail conversion failed\"}"
            } else {
                "{\"error\":\"small AVIF thumbnail conversion failed\"}"
            };
            // Clean up any partial output
            if !jpg_ok { let _ = tokio::fs::remove_file(&jpg_path).await; }
            if !avif_ok { let _ = tokio::fs::remove_file(&avif_path).await; }
            if !sm_avif_ok { let _ = tokio::fs::remove_file(&sm_avif_path).await; }
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(msg))
                .unwrap()
        }
        Err(_) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"processing error\"}"))
                .unwrap()
        }
    }
}

async fn studio_thumbnail_delete(
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

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
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

    let _ = tokio::fs::remove_file(format!("source/{}/thumbnail.avif", mediumid)).await;
    let _ = tokio::fs::remove_file(format!("source/{}/thumbnail.jpg", mediumid)).await;
    let _ = tokio::fs::remove_file(format!("source/{}/thumbnail-sm.avif", mediumid)).await;

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}
