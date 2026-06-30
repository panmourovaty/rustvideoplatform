struct CaptionEntry {
    label: String,
    filename: String,
    is_ass: bool,
}

#[derive(Clone, Copy)]
struct MediumVideoDimensions {
    width: u32,
    height: u32,
}

impl MediumVideoDimensions {
    fn aspect_ratio(&self) -> String {
        format!("{}/{}", self.width, self.height)
    }
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
#[template(path = "pages/medium.html")]
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
    medium_3d_original_ext: String,
    schema_org_json: String,
    config: Config,
    is_logged_in: bool,
    list_id: String,
    list_name: String,
    locale: RequestLocale,
    resolved_lang: String,
}

impl MediumTemplate {
    fn medium_video_dimensions(&self) -> MediumVideoDimensions {
        if self.medium_type == "video" && self.list_id.is_empty() {
            medium_video_dimensions(&self.medium_id)
        } else {
            MediumVideoDimensions {
                width: 1280,
                height: 720,
            }
        }
    }

    fn medium_video_width(&self) -> u32 {
        self.medium_video_dimensions().width
    }

    fn medium_video_height(&self) -> u32 {
        self.medium_video_dimensions().height
    }

    fn medium_video_aspect_ratio(&self) -> String {
        self.medium_video_dimensions().aspect_ratio()
    }
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
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    if !is_valid_resource_id(&mediumid.to_ascii_lowercase()) {
        return Html(Vec::new());
    }
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_logged_in = user.is_some();

    let mediumid = mediumid.to_ascii_lowercase();

    // Fetch media row from ScyllaDB
    let result = db
        .session
        .execute_unpaged(&db.get_media_by_id, (&mediumid,))
        .await;
    let media_row = match result
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(
                String,
                String,
                Option<String>,
                i64,
                String,
                i64,
                String,
                String,
                Option<String>,
            )>()
            .ok()
            .flatten()
        }) {
        Some(r) => r,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let (id, name, _description, upload, owner, views, media_type, visibility, restricted_to_group) =
        media_row;

    // Access control for restricted content
    if !can_access_restricted(
        &db,
        &visibility,
        restricted_to_group.as_deref(),
        &owner,
        &user,
        redis.clone(),
    )
    .await
    {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    // Fetch owner info from users table (separate query since no JOINs in Cassandra)
    let user_result = db
        .session
        .execute_unpaged(&db.get_user_by_login, (&owner,))
        .await;
    let (owner_name, owner_picture) = match user_result
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| {
            rows.maybe_first_row::<(String, Option<String>)>()
                .ok()
                .flatten()
        }) {
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

    let medium_chapters_exist =
        std::path::Path::new(&format!("source/{}/chapters.vtt", medium_id)).exists();
    let medium_previews_exist =
        std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium_id)).exists();
    let is_cmaf =
        std::path::Path::new(&format!("source/{}/video/video.m3u8", medium_id)).exists();

    let medium_video_dimensions = if media_type == "video" {
        medium_video_dimensions(&medium_id)
    } else {
        MediumVideoDimensions {
            width: 1280,
            height: 720,
        }
    };

    let medium_3d_original_ext = if media_type == "object_3d" {
        std::fs::read_to_string(format!("source/{}/original_ext.txt", medium_id))
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        String::new()
    };
    let medium_3d_original_ext = if medium_3d_original_ext.is_empty() {
        "glb".to_owned()
    } else {
        medium_3d_original_ext
    };

    let upload_iso = chrono::DateTime::from_timestamp(upload, 0)
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.to_rfc3339())
        .unwrap_or_default();
    let schema_org_json = {
        let v = match media_type.as_str() {
            "video" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "VideoObject",
                "name": name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.site_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/m/{}/video-sm.mp4", config.site_url, medium_id),
                "embedUrl": format!("{}/m/{}", config.site_url, medium_id),
                "width": medium_video_dimensions.width,
                "height": medium_video_dimensions.height,
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
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.site_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/source/{}/audio.ogg", config.site_url, medium_id),
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
                "contentUrl": format!("{}/source/{}/picture.avif", config.site_url, medium_id),
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
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.site_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            "object_3d" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "3DModel",
                "name": name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.site_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, owner)
                }
            }),
            _ => serde_json::json!({}),
        };
        json_for_html_script(&v)
    };

    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(),
        &config.locale,
        &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "medium".to_owned(), locale.clone());
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
        medium_3d_original_ext,
        schema_org_json,
        config,
        is_logged_in,
        list_id: String::new(),
        list_name: String::new(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

fn medium_video_dimensions(medium_id: &str) -> MediumVideoDimensions {
    let m3u8_path = format!("source/{}/video/video.m3u8", medium_id);
    let mpd_path = format!("source/{}/video/video.mpd", medium_id);

    std::fs::read_to_string(&m3u8_path)
        .ok()
        .and_then(|content| medium_video_dimensions_from_hls(&content))
        .or_else(|| {
            std::fs::read_to_string(&mpd_path)
                .ok()
                .and_then(|content| medium_video_dimensions_from_mpd(&content))
        })
        .unwrap_or(MediumVideoDimensions {
            width: 1280,
            height: 720,
        })
}

fn medium_video_dimensions_from_hls(content: &str) -> Option<MediumVideoDimensions> {
    content
        .lines()
        .filter_map(|line| line.trim().strip_prefix("#EXT-X-STREAM-INF:"))
        .filter_map(|attrs| {
            attrs
                .split(',')
                .find_map(|attr| attr.trim().strip_prefix("RESOLUTION="))
                .and_then(medium_video_dimensions_from_resolution)
        })
        .max_by_key(|dims| (u64::from(dims.width) * u64::from(dims.height), dims.width))
}

fn medium_video_dimensions_from_mpd(content: &str) -> Option<MediumVideoDimensions> {
    let mut in_video_set = false;
    let mut adaptation_dimensions: Option<MediumVideoDimensions> = None;
    let mut best: Option<MediumVideoDimensions> = None;

    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("<AdaptationSet") {
            in_video_set = t.contains(r#"contentType="video""#)
                || t.contains(r#"contentType='video'"#)
                || t.contains(r#"mimeType="video/"#)
                || t.contains(r#"mimeType='video/"#);
            adaptation_dimensions = medium_video_dimensions_from_xml_attrs(t);

            if in_video_set {
                best = medium_video_best_dimensions(best, adaptation_dimensions);
            }
        } else if t.starts_with("</AdaptationSet") {
            in_video_set = false;
            adaptation_dimensions = None;
        } else if t.starts_with("<Representation") {
            let is_video_rep = in_video_set
                || t.contains(r#"mimeType="video/"#)
                || t.contains(r#"mimeType='video/"#);
            if is_video_rep {
                let dimensions =
                    medium_video_dimensions_from_xml_attrs(t).or(adaptation_dimensions);
                best = medium_video_best_dimensions(best, dimensions);
            }
        }
    }

    best
}

fn medium_video_dimensions_from_resolution(value: &str) -> Option<MediumVideoDimensions> {
    let (width, height) = value.split_once('x').or_else(|| value.split_once('X'))?;
    medium_video_dimensions_from_numbers(width, height)
}

fn medium_video_dimensions_from_xml_attrs(element: &str) -> Option<MediumVideoDimensions> {
    let width = medium_video_xml_attr(element, "width")?;
    let height = medium_video_xml_attr(element, "height")?;
    medium_video_dimensions_from_numbers(width, height)
}

fn medium_video_dimensions_from_numbers(
    width: &str,
    height: &str,
) -> Option<MediumVideoDimensions> {
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;

    if width == 0 || height == 0 {
        return None;
    }

    Some(MediumVideoDimensions { width, height })
}

fn medium_video_xml_attr<'a>(element: &'a str, attr: &str) -> Option<&'a str> {
    medium_video_xml_quoted_attr(element, attr, '"')
        .or_else(|| medium_video_xml_quoted_attr(element, attr, '\''))
}

fn medium_video_xml_quoted_attr<'a>(element: &'a str, attr: &str, quote: char) -> Option<&'a str> {
    let pattern = format!("{}={}", attr, quote);
    let mut offset = 0;

    while let Some(relative_start) = element[offset..].find(&pattern) {
        let start = offset + relative_start;
        let is_attr_name_boundary = start == 0
            || element[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || c == '<');

        if is_attr_name_boundary {
            let value_start = start + pattern.len();
            let value_end = value_start + element[value_start..].find(quote)?;
            return Some(&element[value_start..value_end]);
        }

        offset = start + pattern.len();
    }

    None
}

fn medium_video_best_dimensions(
    current: Option<MediumVideoDimensions>,
    candidate: Option<MediumVideoDimensions>,
) -> Option<MediumVideoDimensions> {
    match (current, candidate) {
        (None, candidate) => candidate,
        (current, None) => current,
        (Some(current), Some(candidate)) => {
            let current_area = u64::from(current.width) * u64::from(current.height);
            let candidate_area = u64::from(candidate.width) * u64::from(candidate.height);
            if candidate_area > current_area {
                Some(candidate)
            } else {
                Some(current)
            }
        }
    }
}

#[cfg(test)]
mod medium_video_dimension_tests {
    use super::*;

    #[test]
    fn parses_largest_hls_resolution() {
        let manifest = r#"
#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=854x480
video_480.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2400000,RESOLUTION=1920x1080
video_1080.m3u8
"#;

        let dims = medium_video_dimensions_from_hls(manifest).unwrap();
        assert_eq!(dims.width, 1920);
        assert_eq!(dims.height, 1080);
    }

    #[test]
    fn parses_largest_mpd_video_representation() {
        let manifest = r#"
<MPD>
  <Period>
    <AdaptationSet contentType="video">
      <Representation id="1" bandwidth="700000" width="640" height="360" />
      <Representation id="2" bandwidth="3000000" width="2560" height="1440" />
    </AdaptationSet>
  </Period>
</MPD>
"#;

        let dims = medium_video_dimensions_from_mpd(manifest).unwrap();
        assert_eq!(dims.width, 2560);
        assert_eq!(dims.height, 1440);
    }

    #[test]
    fn parses_stored_medium_description_as_json() {
        let description = r#"{"ops":[{"insert":"A description\n"}]}"#;

        assert_eq!(
            parse_medium_description(description),
            serde_json::json!({
                "ops": [
                    {
                        "insert": "A description\n"
                    }
                ]
            })
        );
    }

    #[test]
    fn returns_empty_delta_for_missing_or_invalid_description() {
        assert_eq!(
            parse_medium_description(""),
            serde_json::json!({ "ops": [] })
        );
        assert_eq!(
            parse_medium_description("not json"),
            serde_json::json!({ "ops": [] })
        );
    }
}

async fn medium_previews_prepare(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    if !can_access_media_request(&headers, &db, redis, &mediumid).await {
        return StatusCode::NOT_FOUND.into_response();
    }
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
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response {
    let mediumid = mediumid.to_ascii_lowercase();
    if !can_access_media_request(&headers, &db, redis, &mediumid).await {
        return StatusCode::NOT_FOUND.into_response();
    }
    let result = db
        .session
        .execute_unpaged(&db.get_media_description, (&mediumid,))
        .await;
    let description = result
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .and_then(|r| r.0)
        .unwrap_or_default();
    Json(parse_medium_description(&description)).into_response()
}

fn parse_medium_description(description: &str) -> serde_json::Value {
    serde_json::from_str(description).unwrap_or_else(|_| serde_json::json!({ "ops": [] }))
}

#[derive(Template)]
#[template(path = "pages/hx-mediumcard.html")]
struct HXMediumCardTemplate {
    media: Vec<Medium>,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
    locale: RequestLocale,
}

#[derive(Template)]
#[template(path = "pages/hx-mediumlist.html")]
struct HXMediumListTemplate {
    current_medium_id: String,
    media: Vec<Medium>,
    config: Config,
    locale: RequestLocale,
}
