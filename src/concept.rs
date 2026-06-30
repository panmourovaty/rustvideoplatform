#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
    r#type: String,
}

async fn concepts(
    Extension(db): Extension<ScyllaDb>,
    Extension(config): Extension<Config>,
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
        active_tab: "concepts".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html")]
struct HXConceptsTemplate {
    concepts: Vec<MediumConcept>,
    locale: RequestLocale,
}
async fn hx_concepts(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let userinfo = get_user_login(headers.clone(), &db, redis.clone()).await.unwrap();

    let rows: Vec<(String, String, String, bool)> = db.session
        .execute_unpaged(&db.get_concepts_by_owner, (&userinfo.login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, String, bool)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let concepts: Vec<MediumConcept> = rows
        .into_iter()
        .map(|(id, name, r#type, processed)| MediumConcept {
            id,
            name,
            processed,
            r#type,
        })
        .collect();

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_request_locale(Some(&userinfo), &common_headers, &db, &localization, &config).await;
    let template = HXConceptsTemplate { concepts, locale };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/concept.html")]
struct ConceptTemplate {
    sidebar: String,
    config: Config,
    current_user: Option<User>,
    concept: MediumConcept,
    owner_groups: Vec<UserGroup>,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn concept(
    Extension(db): Extension<ScyllaDb>,
    Extension(config): Extension<Config>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    Path(conceptid): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    // Query concept by id, then verify owner at application level
    let concept_row = db.session
        .execute_unpaged(&db.get_concept, (&conceptid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, bool)>().ok().flatten());

    let concept_row = match concept_row {
        Some(row) => row,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/studio/concepts\");</script>".to_owned(),
            ));
        }
    };

    let (id, name, r#type, processed) = concept_row;

    // Verify owner by checking the concepts_by_owner table for this user and concept
    let owner_check: Vec<(String, String, String, bool)> = db.session
        .execute_unpaged(&db.get_concepts_by_owner, (&user_info.login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, String, bool)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let is_owner = owner_check.iter().any(|(cid, _, _, _)| cid == &conceptid);
    if !is_owner || !processed {
        return Html(minifi_html(
            "<script>window.location.replace(\"/studio/concepts\");</script>".to_owned(),
        ));
    }

    let concept = MediumConcept {
        id,
        name,
        processed,
        r#type,
    };

    // Fetch user's groups for the dropdown (system groups + user groups)
    let mut owner_groups = system_groups_for_owner(&user_info.login);

    let group_rows: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_groups_by_owner, (&user_info.login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let user_groups: Vec<UserGroup> = group_rows
        .into_iter()
        .map(|(id, name, _created)| UserGroup {
            id,
            name,
            owner: user_info.login.clone(),
        })
        .collect();
    owner_groups.extend(user_groups);

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(
        &config,
        "studio".to_owned(),
        Some(user_info.clone()),
        locale.clone(),
    );
    let template = ConceptTemplate {
        sidebar,
        config,
        current_user: Some(user_info),
        concept,
        owner_groups,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PublishForm {
    medium_id: String,
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}
async fn publish(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(conceptid): Path<String>,
    Form(form): Form<PublishForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    // Query concept by id, then verify owner at application level
    let concept_row = db.session
        .execute_unpaged(&db.get_concept, (&conceptid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, bool)>().ok().flatten());

    let concept_row = match concept_row {
        Some(row) => row,
        None => {
            return Html("<script>window.location.replace(\"/studio/concepts\");</script>".to_owned());
        }
    };

    let (id, name, r#type, processed) = concept_row;

    // Verify owner via concepts_by_owner
    let owner_check: Vec<(String, String, String, bool)> = db.session
        .execute_unpaged(&db.get_concepts_by_owner, (&user_info.login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, String, bool)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let is_owner = owner_check.iter().any(|(cid, _, _, _)| cid == &conceptid);
    if !is_owner || !processed {
        return Html("<script>window.location.replace(\"/studio/concepts\");</script>".to_owned());
    }

    let concept = MediumConcept {
        id,
        name,
        processed,
        r#type,
    };

    let Some(medium_id) = normalize_resource_id(&form.medium_id) else {
        return Html("<b class=\"text-danger\">Invalid media URL identifier.</b>".to_owned());
    };
    if !is_valid_resource_id(&conceptid) {
        return Html("<b class=\"text-danger\">Invalid concept identifier.</b>".to_owned());
    }

    let medium_name = form.medium_name.trim();
    if medium_name.is_empty() || medium_name.len() > 200 {
        return Html(
            "<b class=\"text-danger\">Media name must be between 1 and 200 characters.</b>"
                .to_owned(),
        );
    }

    let existing_media_result = db
        .session
        .execute_unpaged(&db.get_media_owner, (&medium_id,))
        .await;
    let existing_media = match existing_media_result {
        Ok(result) => match result.into_rows_result() {
            Ok(rows) => rows
                .maybe_first_row::<(String,)>()
                .ok()
                .flatten(),
            Err(_) => {
                return Html(
                    "<b class=\"text-danger\">Failed to verify media URL availability.</b>"
                        .to_owned(),
                )
            }
        },
        Err(_) => {
            return Html(
                "<b class=\"text-danger\">Failed to verify media URL availability.</b>"
                    .to_owned(),
            )
        }
    };
    let destination = std::path::Path::new("source").join(&medium_id);
    if existing_media.is_some() || tokio::fs::try_exists(&destination).await.unwrap_or(true) {
        return Html(
            "<b class=\"text-danger\">That media URL is already in use.</b>".to_owned(),
        );
    }

    let visibility = match form.medium_visibility.as_str() {
        "public" | "hidden" | "restricted" => form.medium_visibility.clone(),
        _ => "hidden".to_owned(),
    };
    let restricted_to_group = if visibility == "restricted" {
        let Some(group_id) = form
            .medium_restricted_group
            .as_deref()
            .filter(|group_id| !group_id.is_empty())
        else {
            return Html(
                "<b class=\"text-danger\">Select a group for restricted media.</b>".to_owned(),
            );
        };
        if !is_owned_or_system_group(&db, &user_info.login, group_id).await {
            return Html("<b class=\"text-danger\">Invalid restricted group.</b>".to_owned());
        }
        Some(group_id.to_owned())
    } else {
        None
    };

    let description_value: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or_else(|_| {
            serde_json::json!({ "ops": [] })
        });
    let description = serde_json::to_string(&description_value)
        .unwrap_or_else(|_| "{\"ops\":[]}".to_owned());
    let upload_ts = chrono::Utc::now().timestamp();
    let is_public = visibility == "public";
    let source = std::path::Path::new("upload").join(format!("{conceptid}_processing"));

    if tokio::fs::create_dir(&destination).await.is_err() {
        return Html(
            "<b class=\"text-danger\">That media URL is already in use.</b>".to_owned(),
        );
    }
    if move_dir(
        source.to_string_lossy().as_ref(),
        destination.to_string_lossy().as_ref(),
    )
    .await
    .is_err()
    {
        let _ = tokio::fs::remove_dir_all(&destination).await;
        return Html("<b class=\"text-danger\">Failed to publish processed media.</b>".to_owned());
    }

    let main_insert = db
        .session
        .execute_unpaged(
            &db.insert_media,
            (
                &medium_id,
                medium_name,
                &description,
                &upload_ts,
                &user_info.login,
                &concept.r#type,
                &is_public,
                &visibility,
                &restricted_to_group,
            ),
        )
        .await;
    if main_insert.is_err() {
        let _ = move_dir(
            destination.to_string_lossy().as_ref(),
            source.to_string_lossy().as_ref(),
        )
        .await;
        return Html("<b class=\"text-danger\">Failed to save published media.</b>".to_owned());
    }

    let owner_insert = db
        .session
        .execute_unpaged(
            &db.insert_media_by_owner,
            (
                &user_info.login,
                &upload_ts,
                &medium_id,
                medium_name,
                &description,
                0i64,
                &concept.r#type,
                &is_public,
                &visibility,
                &restricted_to_group,
            ),
        )
        .await;

    if owner_insert.is_err() {
        let _ = db.session.execute_unpaged(&db.delete_media, (&medium_id,)).await;
        let _ = db
            .session
            .execute_unpaged(
                &db.delete_media_by_owner,
                (&user_info.login, upload_ts, &medium_id),
            )
            .await;
        let _ = move_dir(
            destination.to_string_lossy().as_ref(),
            source.to_string_lossy().as_ref(),
        )
        .await;
        return Html("<b class=\"text-danger\">Failed to save published media.</b>".to_owned());
    }

    let cleanup_results = tokio::join!(
        db.session
            .execute_unpaged(&db.delete_concept, (&concept.id,)),
        db.session
            .execute_unpaged(&db.delete_concept_by_owner, (&user_info.login, &concept.id)),
        db.session
            .execute_unpaged(&db.delete_unprocessed_concept, (&concept.id,))
    );
    if cleanup_results.0.is_err() || cleanup_results.1.is_err() || cleanup_results.2.is_err() {
        eprintln!("WARNING: published media {medium_id}, but failed to fully remove its concept");
    }

    Html(format!(
        "<script>window.location.replace(\"/m/{}\");</script>",
        medium_id
    ))
}
