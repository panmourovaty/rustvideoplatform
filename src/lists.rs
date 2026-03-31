#[derive(Serialize, Deserialize, Clone)]
struct List {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ListWithCount {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
    item_count: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct ListModalEntry {
    id: String,
    name: String,
    already_added: bool,
}

#[derive(Deserialize)]
struct CreateListForm {
    name: String,
    visibility: Option<String>,
    restricted_group: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/list.html", escape = "none")]
struct ListPageTemplate {
    sidebar: String,
    config: Config,
    list: List,
    is_owner: bool,
    common_headers: CommonHeaders,
    schema_org_json: String,
    locale: RequestLocale,
    resolved_lang: String,
}

#[derive(Template)]
#[template(path = "pages/hx-listitems.html", escape = "none")]
struct HXListItemsTemplate {
    items: Vec<Medium>,
    list_id: String,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
    locale: RequestLocale,
}

#[derive(Template)]
#[template(path = "pages/hx-listmodal.html", escape = "none")]
struct HXListModalTemplate {
    lists: Vec<ListModalEntry>,
    medium_id: String,
    owner_groups: Vec<UserGroup>,
    locale: RequestLocale,
}

#[derive(Template)]
#[template(path = "pages/hx-userlists.html", escape = "none")]
struct HXUserListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
    locale: RequestLocale,
}

async fn list_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let list_row = db.session.execute_unpaged(&db.get_list_by_id, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, i64)>().ok().flatten());

    let list = match list_row {
        Some((id, name, owner, visibility, restricted_to_group, _created)) => {
            List {
                id,
                name,
                owner,
                visibility,
                restricted_to_group,
            }
        }
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_owner = user_info
        .as_ref()
        .map(|u| u.login == list.owner)
        .unwrap_or(false);

    // Access control for restricted lists
    if !is_owner && !can_access_restricted(&db, &list.visibility, list.restricted_to_group.as_deref(), &list.owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);
    let schema_org_json = serde_json::to_string(&serde_json::json!({
        "@context": "https://schema.org",
        "@type": "ItemList",
        "name": &list.name,
        "url": format!("{}/l/{}", config.site_url, list.id),
        "author": {
            "@type": "Person",
            "identifier": &list.owner,
            "url": format!("{}/u/{}", config.site_url, list.owner)
        }
    })).unwrap_or_default();
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "list".to_owned(), locale.clone());
    let template = ListPageTemplate {
        sidebar,
        config,
        list,
        is_owner,
        common_headers,
        schema_org_json,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn medium_in_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    // Fetch list info
    let list_row = db.session.execute_unpaged(&db.get_list_by_id, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, i64)>().ok().flatten());

    let list = match list_row {
        Some((id, name, owner, visibility, restricted_to_group, _created)) => {
            (id, visibility, restricted_to_group, owner, name)
        }
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_logged_in = user_info.is_some();

    // Access control for restricted lists
    let is_owner = user_info.as_ref().map(|u| u.login == list.3).unwrap_or(false);
    if !is_owner && !can_access_restricted(&db, &list.1, list.2.as_deref(), &list.3, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);

    // Fetch media info (separate query since no JOINs in Cassandra)
    let mediumid_lower = mediumid.to_ascii_lowercase();
    let media_row = db.session.execute_unpaged(&db.get_media_by_id, (&mediumid_lower,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, Option<String>, i64, String, i64, String, String, Option<String>)>().ok().flatten());

    let medium = match media_row {
        Some(r) => r,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let (m_id, m_name, _m_description, m_upload, m_owner, m_views, m_type, m_visibility, m_restricted) = medium;

    if !can_access_restricted(&db, &m_visibility, m_restricted.as_deref(), &m_owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    // Fetch owner info from users table
    let user_result = db.session.execute_unpaged(&db.get_user_by_login, (&m_owner,)).await;
    let (owner_name, owner_picture) = match user_result.ok().and_then(|r| r.into_rows_result().ok()).and_then(|rows| rows.maybe_first_row::<(String, Option<String>)>().ok().flatten()) {
        Some(r) => r,
        None => (m_owner.clone(), None),
    };

    let medium_id = m_id;
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

    let upload_iso = chrono::DateTime::from_timestamp(m_upload, 0)
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.to_rfc3339())
        .unwrap_or_default();
    let schema_org_json = {
        let v = match m_type.as_str() {
            "video" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "VideoObject",
                "name": m_name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/m/{}/video-sm.mp4", config.site_url, medium_id),
                "embedUrl": format!("{}/m/{}", config.site_url, medium_id),
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, m_owner)
                }
            }),
            "audio" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "AudioObject",
                "name": m_name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "contentUrl": format!("{}/source/{}/audio.ogg", config.source_server_url, medium_id),
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, m_owner)
                }
            }),
            "picture" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "ImageObject",
                "name": m_name.clone(),
                "contentUrl": format!("{}/source/{}/picture.avif", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, m_owner)
                }
            }),
            "document_pdf" => serde_json::json!({
                "@context": "https://schema.org",
                "@type": "DigitalDocument",
                "name": m_name.clone(),
                "thumbnailUrl": format!("{}/source/{}/thumbnail.jpg", config.source_server_url, medium_id),
                "uploadDate": upload_iso,
                "author": {
                    "@type": "Person",
                    "name": owner_name.clone(),
                    "url": format!("{}/u/{}", config.site_url, m_owner)
                }
            }),
            _ => serde_json::json!({}),
        };
        serde_json::to_string(&v).unwrap_or_default()
    };

    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "medium".to_owned(), locale.clone());
    let template = MediumTemplate {
        sidebar,
        medium_id,
        medium_name: m_name,
        medium_owner: m_owner,
        medium_owner_name: owner_name,
        medium_owner_picture: owner_picture,
        medium_upload: prettyunixtime(m_upload).await,
        medium_views: m_views,
        medium_type: m_type,
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
        list_id: listid,
        list_name: list.4,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_items(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, db, localization, headers, listid, 0).await
}

async fn hx_list_items_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((listid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, db, localization, headers, listid, page).await
}

async fn hx_list_items_inner(
    config: Config,
    db: ScyllaDb,
    localization: Arc<LocalizationService>,
    headers: HeaderMap,
    listid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let fetch_limit = ((page + 1) * 40 + 1) as i32;
    let skip = (page * 40) as usize;

    // Fetch list items (media_id, position)
    let all_items: Vec<(String, i32)> = db.session.execute_unpaged(&db.get_list_items, (&listid, fetch_limit))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i32)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let page_items: Vec<(String, i32)> = all_items.into_iter().skip(skip).collect();

    let has_more = page_items.len() > 40;
    let page_items: Vec<(String, i32)> = page_items.into_iter().take(40).collect();

    // For each media_id, fetch media info
    let mut items: Vec<Medium> = Vec::new();
    for (media_id, _position) in &page_items {
        let media_row = db.session.execute_unpaged(&db.get_media_by_id, (media_id,)).await
            .ok().and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String, String, Option<String>, i64, String, i64, String, String, Option<String>)>().ok().flatten());

        if let Some((id, name, _desc, _upload, owner, views, media_type, _vis, _rg)) = media_row {
            items.push(Medium {
                id,
                name,
                owner,
                views,
                r#type: media_type,
                sprite_filename: None,
                sprite_x: 0,
                sprite_y: 0,
                visit_time: None,
            });
        }
    }

    let next_page = page + 1;
    let next_url = format!("/hx/listitems/{}/{}", listid, next_page);

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXListItemsTemplate {
        items,
        list_id: listid,
        config,
        page,
        has_more,
        next_url,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_sidebar(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    // Fetch all list items (no pagination for sidebar)
    let list_items: Vec<(String, i32)> = db.session.execute_unpaged(&db.get_list_items, (&listid, 10000i32))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i32)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // For each media_id, fetch media info
    let mut media: Vec<Medium> = Vec::new();
    for (media_id, _position) in &list_items {
        let media_row = db.session.execute_unpaged(&db.get_media_by_id, (media_id,)).await
            .ok().and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String, String, Option<String>, i64, String, i64, String, String, Option<String>)>().ok().flatten());

        if let Some((id, name, _desc, _upload, owner, views, media_type, _vis, _rg)) = media_row {
            media.push(Medium {
                id,
                name,
                owner,
                views,
                r#type: media_type,
                sprite_filename: None,
                sprite_x: 0,
                sprite_y: 0,
                visit_time: None,
            });
        }
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXMediumListTemplate {
        media,
        current_medium_id: mediumid,
        list_id: listid,
        config,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn fetch_lists_and_groups_for_modal(db: &ScyllaDb, user_login: &str, medium_id: &str) -> (Vec<ListModalEntry>, Vec<UserGroup>) {
    // Fetch all lists by owner
    let owner_lists: Vec<(String, String, String, Option<String>, i64)> = db.session.execute_unpaged(&db.get_lists_by_owner, (user_login, 10000i32))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, String, Option<String>, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Fetch which lists contain this medium
    let media_list_entries: Vec<(String, i32)> = db.session.execute_unpaged(&db.get_list_items_by_media, (medium_id,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i32)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let list_ids_set: std::collections::HashSet<String> = media_list_entries.into_iter().map(|(list_id, _)| list_id).collect();

    let lists: Vec<ListModalEntry> = owner_lists.into_iter().map(|(id, name, _vis, _rg, _created)| {
        let already_added = list_ids_set.contains(&id);
        ListModalEntry { id, name, already_added }
    }).collect();

    // Fetch groups
    let mut owner_groups = system_groups_for_owner(user_login);
    let user_groups_rows: Vec<(String, String, i64)> = db.session.execute_unpaged(&db.get_groups_by_owner, (user_login,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    for (id, name, _created) in user_groups_rows {
        owner_groups.push(UserGroup { id, name, owner: user_login.to_string() });
    }

    (lists, owner_groups)
}

async fn hx_list_modal(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let (lists, owner_groups) = fetch_lists_and_groups_for_modal(&db, &user_info.login, &mediumid).await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CreateListForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list_id = generate_medium_id();
    let visibility = match form.visibility.as_deref() {
        Some("public") => "public",
        Some("restricted") => "restricted",
        _ => "hidden",
    };
    let is_public = visibility == "public";
    let restricted_to_group = if visibility == "restricted" {
        form.restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Insert into lists table
    let _ = db.session.execute_unpaged(&db.insert_list, (&list_id, &form.name, &user_info.login, is_public, visibility, &restricted_to_group, created)).await;

    // Insert into lists_by_owner table
    let _ = db.session.execute_unpaged(&db.insert_list_by_owner, (&user_info.login, created, &list_id, &form.name, is_public, visibility, &restricted_to_group)).await;

    // Insert first item into list_items and list_items_by_media
    let _ = db.session.execute_unpaged(&db.insert_list_item, (&list_id, 0i32, &mediumid)).await;
    let _ = db.session.execute_unpaged(&db.insert_list_item_by_media, (&mediumid, &list_id, 0i32)).await;

    // Re-fetch lists and groups for modal template
    let (lists, owner_groups) = fetch_lists_and_groups_for_modal(&db, &user_info.login, &mediumid).await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_to_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    let owner_row = db.session.execute_unpaged(&db.get_list_owner, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Get max position
    let max_pos = db.session.execute_unpaged(&db.get_max_list_position, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(i32,)>().ok().flatten())
        .map(|(p,)| p)
        .unwrap_or(-1);

    let next_pos = max_pos + 1;

    // Insert item
    let _ = db.session.execute_unpaged(&db.insert_list_item, (&listid, next_pos, &mediumid)).await;
    let _ = db.session.execute_unpaged(&db.insert_list_item_by_media, (&mediumid, &listid, next_pos)).await;

    // Re-fetch lists and groups for modal template
    let (lists, owner_groups) = fetch_lists_and_groups_for_modal(&db, &user_info.login, &mediumid).await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_from_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    let owner_row = db.session.execute_unpaged(&db.get_list_owner, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Find position for this media_id in this list
    let media_entries: Vec<(String, i32)> = db.session.execute_unpaged(&db.get_list_items_by_media, (&mediumid,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i32)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    if let Some((_list_id, position)) = media_entries.into_iter().find(|(lid, _)| lid == &listid) {
        // Delete from list_items and list_items_by_media
        let _ = db.session.execute_unpaged(&db.delete_list_item, (&listid, position)).await;
        let _ = db.session.execute_unpaged(&db.delete_list_item_by_media, (&mediumid, &listid)).await;
    }

    // Re-fetch lists and groups for modal template
    let (lists, owner_groups) = fetch_lists_and_groups_for_modal(&db, &user_info.login, &mediumid).await;

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_list(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    // Verify ownership and get created timestamp
    let list_row = db.session.execute_unpaged(&db.get_list_by_id, (&listid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, i64)>().ok().flatten());

    let (owner, created) = match list_row {
        Some((_id, _name, owner, _vis, _rg, created)) => {
            if owner != user_info.login {
                return Html(
                    "<script>window.location.replace(\"/\");</script>".to_owned(),
                );
            }
            (owner, created)
        }
        None => {
            return Html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            );
        }
    };

    // Fetch all items to delete them individually
    let all_items: Vec<(i32, String)> = db.session.execute_unpaged(&db.delete_all_list_items, (&listid,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(i32, String)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Delete each item from both tables
    for (position, media_id) in &all_items {
        let _ = db.session.execute_unpaged(&db.delete_list_item, (&listid, position)).await;
        let _ = db.session.execute_unpaged(&db.delete_list_item_by_media, (media_id, &listid)).await;
    }

    // Delete the list itself
    let _ = db.session.execute_unpaged(&db.delete_list, (&listid,)).await;
    let _ = db.session.execute_unpaged(&db.delete_list_by_owner, (&owner, created, &listid)).await;

    Html("<b class=\"text-success\">LIST DELETED</b><script>window.location.replace(\"/\");</script>".to_owned())
}

async fn hx_user_lists(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(config, db, redis, localization, headers, userid, 0).await
}

async fn hx_user_lists_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(config, db, redis, localization, headers, userid, page).await
}

async fn hx_user_lists_inner(
    config: Config,
    db: ScyllaDb,
    redis: RedisConn,
    localization: Arc<LocalizationService>,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers);
    let user = get_user_login(headers, &db, redis.clone()).await;
    let user_login = user.as_ref().map(|u| u.login.clone()).unwrap_or_default();

    // Fetch all lists by owner with a large limit
    let all_lists: Vec<(String, String, String, Option<String>, i64)> = db.session.execute_unpaged(&db.get_lists_by_owner, (&userid, 10000i32))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, String, Option<String>, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Filter visibility at app level
    let mut visible_lists: Vec<(String, String, String, Option<String>, i64)> = Vec::new();
    for list_row in all_lists {
        let (_, _, ref visibility, ref restricted_to_group, _created) = list_row;
        let is_owner = userid == user_login;
        if is_owner || visibility == "public" || can_access_restricted(&db, visibility, restricted_to_group.as_deref(), &userid, &user, redis.clone()).await {
            visible_lists.push(list_row);
        }
    }

    // Paginate in app code
    let skip = (page * 40) as usize;
    let page_lists: Vec<(String, String, String, Option<String>, i64)> = visible_lists.into_iter().skip(skip).take(41).collect();

    let has_more = page_lists.len() == 41;
    let page_lists: Vec<(String, String, String, Option<String>, i64)> = page_lists.into_iter().take(40).collect();

    // Count items for each list
    let mut lists: Vec<ListWithCount> = Vec::new();
    for (id, name, visibility, restricted_to_group, _created) in page_lists {
        let item_count: i64 = db.session.execute_unpaged(&db.count_list_items, (&id,))
            .await.ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(i32,)>().unwrap().filter_map(|r| r.ok()).count() as i64)
            .unwrap_or(0);

        lists.push(ListWithCount {
            id,
            name,
            owner: userid.clone(),
            visibility,
            restricted_to_group,
            item_count: Some(item_count),
        });
    }

    let next_page = page + 1;
    let next_url = format!("/hx/userlists/{}/{}", userid, next_page);

    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXUserListsTemplate { lists, page, has_more, next_url, locale };
    Html(minifi_html(template.render().unwrap()))
}
