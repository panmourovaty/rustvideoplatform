struct CaptionEntry {
    label: String,
    filename: String,
    is_ass: bool,
}

fn parse_caption_entry(entry: &str) -> CaptionEntry {
    let entry = entry.trim();
    let is_ass = entry.ends_with(".ass") || entry.ends_with(".ssa");
    if let Some(dot_pos) = entry.rfind('.') {
        CaptionEntry {
            label: entry[..dot_pos].to_string(),
            filename: entry.to_string(),
            is_ass,
        }
    } else {
        CaptionEntry {
            label: entry.to_string(),
            filename: format!("{}.vtt", entry),
            is_ass: false,
        }
    }
}

#[derive(Template)]
#[template(path = "pages/medium.html", escape = "none")]
struct MediumTemplate {
    sidebar: String,
    medium_id: String,
    medium_name: String,
    medium_owner: String,
    medium_owner_name: String,
    medium_owner_picture: Option<String>,
    medium_upload: String,
    medium_views: i64,
    medium_type: String,
    medium_captions_exist: bool,
    medium_captions_list: Vec<CaptionEntry>,
    medium_has_ass_captions: bool,
    medium_custom_font: bool,
    medium_chapters_exist: bool,
    medium_previews_exist: bool,
    is_cmaf: bool,
    schema_org_json: String,
    config: Config,
    common_headers: CommonHeaders,
    is_logged_in: bool,
    list_id: String,
    list_name: String,
}

#[derive(Serialize, Deserialize)]
struct Medium {
    id: String,
    name: String,
    owner: String,
    views: i64,
    r#type: String,
    sprite_filename: Option<String>,
    sprite_x: i32,
    sprite_y: i32,
    visit_time: Option<String>,
}

async fn medium(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_logged_in = user.is_some();

    let mediumid = mediumid.to_ascii_lowercase();

    // Fetch media row from ScyllaDB
    let result = db.session.execute_unpaged(&db.get_media_by_id, (&mediumid,)).await;
    let media_row = match result.ok().and_then(|r| r.into_rows_result().ok()).and_then(|rows| rows.maybe_first_row::<(String, String, Option<String>, i64, String, i64, String, String, Option<String>)>().ok().flatten()) {
        Some(r) => r,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let (id, name, _description, upload, owner, views, media_type, visibility, restricted_to_group) = media_row;

    // Access control for restricted content
    if !can_access_restricted(&db, &visibility, restricted_to_group.as_deref(), &owner, &user, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    // Fetch owner info from users table (separate query since no JOINs in Cassandra)
    let user_result = db.session.execute_unpaged(&db.get_user_by_login, (&owner,)).await;
    let (owner_name, owner_picture) = match user_result.ok().and_then(|r| r.into_rows_result().ok()).and_then(|rows| rows.maybe_first_row::<(String, Option<String>)>().ok().flatten()) {
        Some(r) => r,
        None => (owner.clone(), None),
    };

    let common_headers = extract_common_headers(&headers);

    let medium_id = id;
    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<CaptionEntry> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium_id)).exists() {
        medium_captions_exist = true;
        for entry in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium_id)) {
            if !entry.trim().is_empty() {
                medium_captions_list.push(parse_caption_entry(&entry));
            }
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_has_ass_captions = medium_captions_list.iter().any(|c| c.is_ass);
    let medium_custom_font =
        std::path::Path::new(&format!("source/{}/captions/font.woff2", medium_id)).exists();

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", medium_id)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium_id)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let is_cmaf: bool;
    if std::path::Path::new(&format!("source/{}/video/video.m3u8", medium_id)).exists() {
        is_cmaf = true;
    } else {
        is_cmaf = false;
    }

    let upload_iso = chrono::DateTime::from_timestamp(upload, 0)
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.to_rfc3339())
        .unwrap_or_default();
    let schema_org_json = {
        let v = match media_type.as_str() {
            "video" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "VideoObject",
                "name": name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/m/{}/video-sm.mp4", config.site_url, medium_id),
                "embedUrl": format!("{}/m/{}", config.site_url, medium_id),
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            "audio" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "AudioObject",
                "name": name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/source/{}/audio.ogg", config.source_server_url, medium_id),
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            "picture" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "ImageObject",
                "name": name.clone(),
                "contentUrl": format!("{}/source/{}/picture.avif", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            "document_pdf" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "DigitalDocument",
                "name": name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            _ => serde_json::json!({}),
        };
        serde_json::to_string(&v).unwrap_or_default()
    };

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id,
        medium_name: name,
        medium_owner: owner,
        medium_owner_name: owner_name,
        medium_owner_picture: owner_picture,
        medium_upload: prettyunixtime(upload).await,
        medium_views: views,
        medium_type: media_type,
        medium_captions_exist,
        medium_captions_list,
        medium_has_ass_captions,
        medium_custom_font,
        medium_chapters_exist,
        medium_previews_exist,
        is_cmaf,
        schema_org_json,
        config,
        common_headers,
        is_logged_in,
        list_id: String::new(),
        list_name: String::new(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn medium_previews_prepare(Path(mediumid): Path<String>) -> Response<Body> {
    let source_file_path = format!("source/{}/previews/previews.vtt", mediumid);

    match tokio::fs::read_to_string(&source_file_path).await {
        Ok(vtt_content) => {
            let fixed_vtt = fix_vtt_urls(&vtt_content, &mediumid);
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "text/vtt")
                .body(Body::from(fixed_vtt))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

fn fix_vtt_urls(vtt_content: &str, mediumid: &str) -> String {
    let base_path = format!("/source/{}/previews/", mediumid);

    vtt_content
        .lines()
        .map(|line| {
            let trimmed = line.trim();

            if trimmed.is_empty()
                || trimmed.starts_with("WEBVTT")
                || trimmed.contains("-->")
                || trimmed.starts_with("NOTE")
            {
                return line.to_string();
            }

            let path_part = trimmed.split('#').next().unwrap_or(trimmed);
            let is_avif = path_part.to_lowercase().ends_with(".avif");
            let is_relative = !trimmed.starts_with('/')
                && !trimmed.starts_with("http://")
                && !trimmed.starts_with("https://");

            if is_avif && is_relative {
                if let Some(hash_pos) = trimmed.find('#') {
                    let (path, fragment) = trimmed.split_at(hash_pos);
                    return format!("{}{}{}", base_path, path, fragment);
                } else {
                    return format!("{}{}", base_path, trimmed);
                }
            }

            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn medium_description_prepare(
    Extension(db): Extension<ScyllaDb>,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let result = db.session.execute_unpaged(&db.get_media_description, (&mediumid.to_ascii_lowercase(),)).await;
    let description = result.ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .and_then(|r| r.0)
        .unwrap_or_default();
    Json(serde_json::Value::String(description))
}

#[derive(Template)]
#[template(path = "pages/hx-mediumcard.html", escape = "none")]
struct HXMediumCardTemplate {
    media: Vec<Medium>,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}

#[derive(Template)]
#[template(path = "pages/hx-mediumlist.html", escape = "none")]
struct HXMediumListTemplate {
    current_medium_id: String,
    list_id: String,
    media: Vec<Medium>,
    config: Config,
}
