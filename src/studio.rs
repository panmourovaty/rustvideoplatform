#[derive(Template)]
#[template(path = "pages/studio.html", escape = "none")]
struct StudioTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    active_tab: String,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn studio(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "studio".to_owned(), locale.clone());
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "media".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumStudio {
    id: String,
    name: String,
    description: Option<serde_json::Value>,
    views: i64,
    r#type: String,
}
#[derive(Template)]
#[template(path = "pages/hx-studio.html", escape = "none")]
struct HXStudioTemplate {
    media: Vec<MediumStudio>,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, db, redis, headers, 0).await
}

async fn hx_studio_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, db, redis, headers, page).await
}

async fn hx_studio_inner(
    config: Config,
    db: ScyllaDb,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let skip = (page * 40) as usize;
    let fetch_limit = ((page + 1) * 40 + 1) as i32;

    // Fetch from media_by_owner (already ordered by upload DESC via clustering key)
    // Row type: (id, name, description, views, type, upload, visibility, restricted_to_group)
    let all_rows: Vec<(String, String, Option<String>, i64, String, i64, String, Option<String>)> =
        db.session.execute_unpaged(&db.get_media_by_owner, (&user_info.login, fetch_limit))
            .await
            .ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
            .unwrap_or_default();

    // App-level pagination: skip and take
    let remaining: Vec<_> = all_rows.into_iter().skip(skip).collect();
    let has_more = remaining.len() > 40;
    let media: Vec<MediumStudio> = remaining.into_iter().take(40).map(|(id, name, description, views, media_type, _upload, _visibility, _restricted_to_group)| {
        MediumStudio {
            id,
            name,
            description: description.and_then(|d| serde_json::from_str(&d).ok()),
            views,
            r#type: media_type,
        }
    }).collect();

    let next_page = page + 1;
    let next_url = format!("/hx/studio/{}", next_page);

    let template = HXStudioTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

async fn studio_lists(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "studio".to_owned(), locale.clone());
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "lists".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-lists.html", escape = "none")]
struct HXStudioListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio_lists(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(db, redis, headers, 0).await
}

async fn hx_studio_lists_page(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(db, redis, headers, page).await
}

async fn hx_studio_lists_inner(
    db: ScyllaDb,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let skip = (page * 40) as usize;
    let fetch_limit = ((page + 1) * 40 + 1) as i32;

    // Fetch lists from lists_by_owner (already ordered by created DESC via clustering key)
    // Row type: (id, name, visibility, restricted_to_group, created)
    let all_rows: Vec<(String, String, String, Option<String>, i64)> =
        db.session.execute_unpaged(&db.get_lists_by_owner, (&user_info.login, fetch_limit))
            .await
            .ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String, String, String, Option<String>, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
            .unwrap_or_default();

    // App-level pagination: skip and take
    let remaining: Vec<_> = all_rows.into_iter().skip(skip).collect();
    let has_more = remaining.len() > 40;
    let page_rows: Vec<_> = remaining.into_iter().take(40).collect();

    // For each list, count items using count_list_items (count rows at app level)
    let mut lists: Vec<ListWithCount> = Vec::new();
    for (id, name, visibility, restricted_to_group, _created) in page_rows {
        let item_count: i64 = db.session.execute_unpaged(&db.count_list_items, (&id,))
            .await
            .ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(i32,)>().unwrap().filter_map(|r| r.ok()).count() as i64)
            .unwrap_or(0);

        lists.push(ListWithCount {
            id,
            name,
            owner: user_info.login.clone(),
            visibility,
            restricted_to_group,
            item_count: Some(item_count),
        });
    }

    let next_page = page + 1;
    let next_url = format!("/hx/studio/lists/{}", next_page);

    let template = HXStudioListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumEdit {
    id: String,
    name: String,
    visibility: String,
    restricted_to_group: String,
    medium_type: String,
}
#[derive(Template)]
#[template(path = "pages/studio-edit.html", escape = "none")]
struct StudioEditTemplate {
    sidebar: String,
    config: Config,
    medium: MediumEdit,
    common_headers: CommonHeaders,
    active_tab: String,
    locale: RequestLocale,
    resolved_lang: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-description.html", escape = "none")]
struct HXStudioEditDescriptionTemplate {
    medium: MediumEdit,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-chapters.html", escape = "none")]
struct HXStudioEditChaptersTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-subtitles.html", escape = "none")]
struct HXStudioEditSubtitlesTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-thumbnail.html", escape = "none")]
struct HXStudioEditThumbnailTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-danger.html", escape = "none")]
struct HXStudioEditDangerTemplate {
    medium_id: String,
    medium_name: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-permissions.html", escape = "none")]
struct HXStudioEditPermissionsTemplate {
    medium: MediumEdit,
    owner_groups: Vec<UserGroup>,
}
async fn studio_edit(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    // Row type: (id, name, owner, visibility, restricted_to_group, type)
    let medium = db.session.execute_unpaged(&db.get_media_basic, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

    match medium {
        Some((id, name, owner, visibility, restricted_to_group, media_type)) => {
            if owner != user_info.login {
                return Html(minifi_html(
                    "<script>window.location.replace(\"/studio\");</script>".to_owned(),
                ));
            }

            let common_headers = extract_common_headers(&headers);
            let locale = resolve_locale_noauth(
                common_headers.accept_language.as_deref(), &config.locale, &localization,
            );
            let resolved_lang = locale.lang.clone();
            let sidebar = generate_sidebar(&config, "studio".to_owned(), locale.clone());
            let template = StudioEditTemplate {
                sidebar,
                config,
                medium: MediumEdit {
                    id,
                    name,
                    visibility,
                    restricted_to_group: restricted_to_group.unwrap_or_default(),
                    medium_type: media_type,
                },
                common_headers,
                active_tab: "description".to_owned(),
                locale,
                resolved_lang,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html(
            "<script>window.location.replace(\"/studio\");</script>".to_owned(),
        )),
    }
}

#[derive(Serialize, Deserialize)]
struct EditForm {
    medium_name: String,
    medium_description: String,
}

#[derive(Serialize, Deserialize)]
struct PermissionsEditForm {
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}
async fn studio_edit_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<EditForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    let media_owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match media_owner {
        Some((owner,)) => {
            if owner != user_info.login {
                return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
            }
        }
        None => {
            return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
        }
    }

    // Serialize description JSON to string for Cassandra storage
    let description: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or(serde_json::Value::Null);
    let description_string = serde_json::to_string(&description).unwrap_or_else(|_| "null".to_owned());

    // Update media table
    let update_result = db.session.execute_unpaged(
        &db.update_media_name_desc,
        (&form.medium_name, &description_string, &mediumid),
    ).await;

    if update_result.is_err() {
        return Html(format!(
            "<b class=\"text-danger\">Failed to save changes.</b><script>setTimeout(function(){{window.location.replace(\"/studio/edit/{}\");}},2000);</script>",
            mediumid
        ));
    }

    Html(format!(
        "<script>window.location.replace(\"/studio/edit/{}\");</script>",
        mediumid
    ))
}

async fn hx_delete_video(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Redirect::to("/login");
    }
    let user_info = user_info.unwrap();

    // Fetch full media row to get owner and upload time (needed for media_by_owner deletion)
    // Row type: (id, name, description, upload, owner, views, type, visibility, restricted_to_group)
    let media_row = db.session.execute_unpaged(&db.get_media_by_id, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, Option<String>, i64, String, i64, String, String, Option<String>)>().ok().flatten());

    let (_id, _name, _description, upload, owner, _views, _media_type, _visibility, _restricted_to_group) = match media_row {
        Some(r) => r,
        None => return Redirect::to("/studio"),
    };

    if owner != user_info.login {
        return Redirect::to("/studio");
    }

    // Delete from lists: find all lists containing this media, then delete each entry
    let list_entries: Vec<(String, i32)> = db.session.execute_unpaged(&db.get_list_items_by_media, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, i32)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for (list_id, position) in &list_entries {
        let _ = db.session.execute_unpaged(&db.delete_list_item, (list_id, position)).await;
        let _ = db.session.execute_unpaged(&db.delete_list_item_by_media, (&mediumid, list_id)).await;
    }

    // Delete media from main table and media_by_owner
    let _ = db.session.execute_unpaged(&db.delete_media, (&mediumid,)).await;
    let _ = db.session.execute_unpaged(&db.delete_media_by_owner, (&owner, &upload, &mediumid)).await;

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    Redirect::to("/studio")
}

async fn hx_studio_edit_description(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Row type: (id, name, owner, visibility, restricted_to_group, type)
    let medium = db.session.execute_unpaged(&db.get_media_basic, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

    match medium {
        Some((id, name, owner, visibility, restricted_to_group, media_type)) => {
            if owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }

            let template = HXStudioEditDescriptionTemplate {
                medium: MediumEdit {
                    id,
                    name,
                    visibility,
                    restricted_to_group: restricted_to_group.unwrap_or_default(),
                    medium_type: media_type,
                },
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn hx_studio_edit_chapters_tab(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
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

    let template = HXStudioEditChaptersTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_subtitles_tab(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
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

    let template = HXStudioEditSubtitlesTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_thumbnail_tab(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
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

    let template = HXStudioEditThumbnailTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_danger_tab(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Use get_media_basic to get owner and name
    // Row type: (id, name, owner, visibility, restricted_to_group, type)
    let row = db.session.execute_unpaged(&db.get_media_basic, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

    match row {
        Some((_id, medium_name, owner, _visibility, _restricted_to_group, _media_type)) => {
            if owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }
            let template = HXStudioEditDangerTemplate { medium_id: mediumid, medium_name };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn hx_studio_edit_permissions_tab(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Row type: (id, name, owner, visibility, restricted_to_group, type)
    let medium = db.session.execute_unpaged(&db.get_media_basic, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, String, Option<String>, String)>().ok().flatten());

    match medium {
        Some((id, name, owner, visibility, restricted_to_group, media_type)) => {
            if owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }

            let mut owner_groups = system_groups_for_owner(&user_info.login);
            // Fetch user groups from user_groups_by_owner
            // Row type: (id, name, created)
            let user_groups: Vec<UserGroup> = db.session.execute_unpaged(&db.get_groups_by_owner, (&user_info.login,))
                .await
                .ok().and_then(|r| r.into_rows_result().ok())
                .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).map(|(id, name, _created)| {
                    UserGroup {
                        id,
                        name,
                        owner: user_info.login.clone(),
                    }
                }).collect::<Vec<_>>())
                .unwrap_or_default();
            owner_groups.extend(user_groups);

            let template = HXStudioEditPermissionsTemplate {
                medium: MediumEdit {
                    id,
                    name,
                    visibility,
                    restricted_to_group: restricted_to_group.unwrap_or_default(),
                    medium_type: media_type,
                },
                owner_groups,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn studio_edit_permissions_save(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<PermissionsEditForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    let media_owner = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match media_owner {
        Some((owner,)) => {
            if owner != user_info.login {
                return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
            }
        }
        None => {
            return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
        }
    }

    let visibility = match form.medium_visibility.as_str() {
        "public" | "hidden" | "restricted" => form.medium_visibility.clone(),
        _ => "hidden".to_owned(),
    };
    let ispublic = visibility == "public";
    let restricted_to_group = if visibility == "restricted" {
        form.medium_restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    // Update media table permissions
    let update_result = db.session.execute_unpaged(
        &db.update_media_permissions,
        (&ispublic, &visibility, &restricted_to_group, &mediumid),
    ).await;

    if update_result.is_err() {
        return Html(format!(
            "<b class=\"text-danger\">Failed to save changes.</b><script>setTimeout(function(){{window.location.replace(\"/studio/edit/{}\");}},2000);</script>",
            mediumid
        ));
    }

    Html(format!(
        "<script>window.location.replace(\"/studio/edit/{}\");</script>",
        mediumid
    ))
}
