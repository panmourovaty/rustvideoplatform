/// Parses an HLS master playlist and returns the URI of the variant with either the lowest
/// or highest BANDWIDTH. Returns the master path unchanged if it is not a master playlist
/// or cannot be read.
fn hls_variant_for_quality(master_path: &str, lowest: bool) -> String {
    let Ok(content) = std::fs::read_to_string(master_path) else {
        return master_path.to_string();
    };
    if !content.contains("#EXT-X-STREAM-INF:") {
        return master_path.to_string();
    }
    let base = std::path::Path::new(master_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();

    let mut best: Option<(u64, String)> = None;
    let mut pending_bw: Option<u64> = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some(stream_info) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            pending_bw = stream_info
                .split(',')
                .find_map(|attr| attr.trim().strip_prefix("BANDWIDTH=")?.parse::<u64>().ok());
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some(bw) = pending_bw.take() {
                let uri = if line.starts_with('/') || line.contains("://") {
                    line.to_string()
                } else {
                    base.join(line).to_string_lossy().into_owned()
                };
                let is_better = best
                    .as_ref()
                    .is_none_or(|(b, _)| if lowest { bw < *b } else { bw > *b });
                if is_better {
                    best = Some((bw, uri));
                }
            }
        }
    }

    best.map(|(_, p)| p).unwrap_or_else(|| master_path.to_string())
}

/// Parses a DASH MPD and returns the 0-based index (as FFmpeg numbers it) of the video stream
/// with the lowest bandwidth. Returns 0 on parse failure or when only one stream exists.
fn mpd_lowest_video_stream_idx(mpd_path: &str) -> usize {
    let Ok(content) = std::fs::read_to_string(mpd_path) else {
        return 0;
    };
    let mut in_video_set = false;
    let mut stream_idx = 0usize;
    let mut best: Option<(u64, usize)> = None;

    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("<AdaptationSet") {
            in_video_set = t.contains(r#"contentType="video""#)
                || t.contains(r#"mimeType="video/"#);
        } else if t.starts_with("</AdaptationSet") {
            in_video_set = false;
        } else if in_video_set && t.starts_with("<Representation") {
            if let Some(bw) = xml_attr(t, "bandwidth").and_then(|v| v.parse::<u64>().ok()) {
                if best.is_none_or(|(b, _)| bw < b) {
                    best = Some((bw, stream_idx));
                }
                stream_idx += 1;
            }
        }
    }

    best.map(|(_, i)| i).unwrap_or(0)
}

fn xml_attr<'a>(element: &'a str, attr: &str) -> Option<&'a str> {
    let pat = [attr, "=\""].concat();
    let start = element.find(pat.as_str())? + pat.len();
    let end = start + element[start..].find('"')?;
    Some(&element[start..end])
}

async fn stream_video_as_mp4(
    db: &ScyllaDb,
    redis: RedisConn,
    headers: HeaderMap,
    medium_id: &str,
    lowest_quality: bool,
) -> Response<Body> {
    if !is_valid_resource_id(medium_id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let medium_row = db.session.execute_unpaged(&db.get_media_basic, (medium_id,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

    let (_, name, owner, visibility, restricted_to_group, medium_type) = match medium_row {
        Some(row) => row,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    };

    if medium_type != "video" {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }

    let user = get_user_login(headers, db, redis.clone()).await;

    if !can_access_restricted(
        db,
        &visibility,
        restricted_to_group.as_deref(),
        &owner,
        &user,
        redis.clone(),
    )
    .await
    {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::empty())
            .unwrap();
    }

    let m3u8_path = format!("source/{}/video/video.m3u8", medium_id);
    let mpd_path = format!("source/{}/video/video.mpd", medium_id);

    let (input_path, is_hls) = if std::path::Path::new(&m3u8_path).exists() {
        let path = if lowest_quality {
            hls_variant_for_quality(&m3u8_path, true)
        } else {
            m3u8_path.clone()
        };
        (path, true)
    } else if std::path::Path::new(&mpd_path).exists() {
        (mpd_path.clone(), false)
    } else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    };

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.arg("-hide_banner").arg("-loglevel").arg("error");

    if is_hls {
        cmd.arg("-protocol_whitelist")
            .arg("file,pipe,crypto,data")
            .arg("-allowed_extensions")
            .arg("ALL");
    }

    cmd.arg("-i").arg(&input_path);

    // For HLS lowest quality, the variant playlist may be video-only (audio in a
    // separate rendition), so add the master playlist as a second input for audio.
    if is_hls && lowest_quality {
        cmd.arg("-protocol_whitelist")
            .arg("file,pipe,crypto,data")
            .arg("-allowed_extensions")
            .arg("ALL")
            .arg("-i")
            .arg(&m3u8_path);
        cmd.arg("-map").arg("0:v:0")
            .arg("-map").arg("1:a:0?");
    } else if is_hls {
        // Full quality HLS: explicitly map video and audio from the master playlist.
        cmd.arg("-map").arg("0:v:0")
            .arg("-map").arg("0:a:0?");
    } else if !is_hls && lowest_quality {
        // DASH with lowest quality: map the lowest-bandwidth video stream and first audio.
        let idx = mpd_lowest_video_stream_idx(&mpd_path);
        cmd.arg("-map").arg(format!("0:v:{}", idx))
            .arg("-map").arg("0:a:0?");
    } else {
        // DASH full quality: explicitly map video and first audio.
        cmd.arg("-map").arg("0:v:0")
            .arg("-map").arg("0:a:0?");
    }

    cmd.arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("frag_keyframe+empty_moov")
        .arg("-f")
        .arg("mp4")
        .arg("pipe:1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to start video composition"))
                .unwrap()
        }
    };

    let stdout = child.stdout.take().unwrap();
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(stdout));

    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    let safe_filename: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let disposition_filename = if lowest_quality {
        format!("{}-sm.mp4", safe_filename)
    } else {
        format!("{}.mp4", safe_filename)
    };

    Response::builder()
        .header("Content-Type", "video/mp4")
        .header(
            "Content-Disposition",
            format!("inline; filename=\"{}\"", disposition_filename),
        )
        .body(body)
        .unwrap()
}

async fn compose_mp4_sm(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    stream_video_as_mp4(&db, redis, headers, &mediumid.to_ascii_lowercase(), true).await
}

async fn compose_mp4(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    stream_video_as_mp4(&db, redis, headers, &mediumid.to_ascii_lowercase(), false).await
}
