#[derive(Serialize, Deserialize, Clone)]
struct ChapterData {
    start: String,
    title: String,
}

async fn studio_chapters_get(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    if !is_valid_resource_id(&mediumid) {
        return Json(serde_json::Value::Array(vec![]));
    }
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::Value::Array(vec![]));
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Json(serde_json::Value::Array(vec![])),
    }

    let vtt_path = format!("source/{}/chapters.vtt", mediumid);
    match tokio::fs::read_to_string(&vtt_path).await {
        Ok(content) => {
            let chapters = parse_webvtt_chapters(&content);
            Json(serde_json::to_value(chapters).unwrap_or(serde_json::Value::Array(vec![])))
        }
        Err(_) => Json(serde_json::Value::Array(vec![])),
    }
}

async fn studio_chapters_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Json(chapters): Json<Vec<ChapterData>>,
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

    let vtt_path = format!("source/{}/chapters.vtt", mediumid);

    if chapters.is_empty() {
        let _ = tokio::fs::remove_file(&vtt_path).await;
        return Response::builder()
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"ok\":true}"))
            .unwrap();
    }

    let vtt_content = generate_webvtt_from_chapters(&chapters);
    match tokio::fs::write(&vtt_path, vtt_content).await {
        Ok(_) => Response::builder()
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"ok\":true}"))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to save\"}"))
            .unwrap(),
    }
}

fn parse_webvtt_chapters(content: &str) -> Vec<ChapterData> {
    let mut chapters = Vec::new();
    let mut lines = content.lines().peekable();

    // Skip until WEBVTT header
    let mut found_header = false;
    for line in lines.by_ref() {
        if line.trim().starts_with("WEBVTT") {
            found_header = true;
            break;
        }
    }
    if !found_header {
        return chapters;
    }

    loop {
        // Find next timestamp line, skipping empty lines and cue identifiers
        let mut timestamp_line = None;
        for line in lines.by_ref() {
            let trimmed = line.trim();
            if trimmed.contains("-->") {
                timestamp_line = Some(trimmed.to_string());
                break;
            }
        }

        let Some(ts_line) = timestamp_line else {
            break;
        };

        let parts: Vec<&str> = ts_line.split("-->").collect();
        if parts.len() != 2 {
            continue;
        }

        let start = parts[0].trim().to_string();

        // Collect title lines until empty line or end
        let mut title = String::new();
        while let Some(line) = lines.peek() {
            if line.trim().is_empty() {
                break;
            }
            if !title.is_empty() {
                title.push(' ');
            }
            title.push_str(lines.next().unwrap().trim());
        }

        if !title.is_empty() {
            chapters.push(ChapterData { start, title });
        }
    }

    chapters
}

fn timestamp_to_seconds(ts: &str) -> f64 {
    let ts = ts.trim();
    let (time_part, ms_str) = if let Some(dot_pos) = ts.rfind('.') {
        (&ts[..dot_pos], &ts[dot_pos + 1..])
    } else {
        (ts, "0")
    };

    let parts: Vec<&str> = time_part.split(':').collect();
    let (h, m, s) = match parts.len() {
        3 => (
            parts[0].parse::<f64>().unwrap_or(0.0),
            parts[1].parse::<f64>().unwrap_or(0.0),
            parts[2].parse::<f64>().unwrap_or(0.0),
        ),
        2 => (
            0.0,
            parts[0].parse::<f64>().unwrap_or(0.0),
            parts[1].parse::<f64>().unwrap_or(0.0),
        ),
        1 => (
            0.0,
            0.0,
            parts[0].parse::<f64>().unwrap_or(0.0),
        ),
        _ => (0.0, 0.0, 0.0),
    };

    let ms = ms_str.parse::<f64>().unwrap_or(0.0)
        / 10f64.powi(ms_str.len() as i32);

    h * 3600.0 + m * 60.0 + s + ms
}

fn normalize_vtt_timestamp(ts: &str) -> String {
    let seconds = timestamp_to_seconds(ts);
    let total_secs = seconds.floor() as u64;
    let ms = ((seconds - seconds.floor()) * 1000.0).round() as u64;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}

fn generate_webvtt_from_chapters(chapters: &[ChapterData]) -> String {
    let mut sorted: Vec<ChapterData> = chapters.to_vec();
    sorted.sort_by(|a, b| {
        timestamp_to_seconds(&a.start)
            .partial_cmp(&timestamp_to_seconds(&b.start))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut vtt = String::from("WEBVTT\n\n");

    for i in 0..sorted.len() {
        let start = normalize_vtt_timestamp(&sorted[i].start);
        let end = if i + 1 < sorted.len() {
            normalize_vtt_timestamp(&sorted[i + 1].start)
        } else {
            "99:59:59.000".to_string()
        };

        vtt.push_str(&format!(
            "{} --> {}\n{}\n\n",
            start, end, sorted[i].title
        ));
    }

    vtt
}
