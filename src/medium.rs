#[derive(Template)]
#[template(path = "pages/medium.html", escape = "none")]
struct MediumTemplate {
    sidebar: String,
    medium_id: String,
    medium_name: String,
    medium_owner: String,
    medium_likes: i64,
    medium_dislikes: i64,
    medium_upload: String,
    medium_views: i64,
    medium_type: String,
    medium_captions_exist: bool,
    medium_captions_list: Vec<String>,
    medium_chapters_exist: bool,
    medium_previews_exist: bool,
    medium_document_filename: String,
    config: Config,
    common_headers: CommonHeaders,
    is_logged_in: bool,
    list_id: String,
    list_name: String,
}

#[derive(Template)]
#[template(path = "pages/zetaoffice-viewer.html", escape = "none")]
struct ZetaOfficeViewerTemplate {
    medium_id: String,
    document_filename: String,
}

#[derive(Serialize, Deserialize)]
struct Medium {
    id: String,
    name: String,
    owner: String,
    views: i64,
    r#type: String,
}

async fn medium(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let is_logged_in = is_logged(get_user_login(headers.clone(), &pool, session_store).await).await;
    let common_headers = extract_common_headers(&headers).unwrap();
    let medium = sqlx::query!(
        "SELECT id,name,description,upload,owner,likes,dislikes,views,type FROM media WHERE id=$1;",
        mediumid.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<String> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium.id)).exists() {
        medium_captions_exist = true;
        for caption_name in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium.id)) {
            medium_captions_list.push(caption_name);
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", medium.id)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium.id)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let medium_document_filename = match medium.r#type.as_str() {
        "document_writer" => "document.odt".to_owned(),
        "document_spreadsheet" => "document.ods".to_owned(),
        "document_presentation" => "document.odp".to_owned(),
        _ => String::new(),
    };

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id: medium.id,
        medium_name: medium.name,
        medium_owner: medium.owner,
        medium_likes: medium.likes,
        medium_dislikes: medium.dislikes,
        medium_upload: prettyunixtime(medium.upload).await,
        medium_views: medium.views,
        medium_type: medium.r#type,
        medium_captions_exist,
        medium_captions_list,
        medium_chapters_exist,
        medium_previews_exist,
        medium_document_filename,
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
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    Json(
        sqlx::query!(
            "SELECT description FROM media WHERE id=$1;",
            mediumid.to_ascii_lowercase()
        )
        .fetch_one(&pool)
        .await
        .expect("Database error")
        .description
        .unwrap_or_default(),
    )
}

#[derive(Template)]
#[template(path = "pages/hx-mediumcard.html", escape = "none")]
struct HXMediumCardTemplate {
    media: Vec<Medium>,
    config: Config,
}

#[derive(Template)]
#[template(path = "pages/hx-mediumlist.html", escape = "none")]
struct HXMediumListTemplate {
    current_medium_id: String,
    media: Vec<Medium>,
    config: Config,
}

async fn zetaoffice_viewer(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id,type FROM media WHERE id=$1;"
    )
    .bind(mediumid.to_ascii_lowercase())
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let document_filename = match row.1.as_str() {
        "document_writer" => "document.odt",
        "document_spreadsheet" => "document.ods",
        "document_presentation" => "document.odp",
        _ => "",
    };

    let template = ZetaOfficeViewerTemplate {
        medium_id: row.0,
        document_filename: document_filename.to_owned(),
    };

    Response::builder()
        .header("Content-Type", "text/html")
        .header("Cross-Origin-Opener-Policy", "same-origin")
        .header("Cross-Origin-Embedder-Policy", "require-corp")
        .body(Body::from(minifi_html(template.render().unwrap())))
        .unwrap()
}
