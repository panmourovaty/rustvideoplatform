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
        active_tab: "concepts".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html", escape = "none")]
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
#[template(path = "pages/concept.html", escape = "none")]
struct ConceptTemplate {
    sidebar: String,
    config: Config,
    concept: MediumConcept,
    common_headers: CommonHeaders,
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
    let sidebar = generate_sidebar(&config, "studio".to_owned(), locale.clone());
    let template = ConceptTemplate {
        sidebar,
        config,
        concept,
        common_headers,
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

    let concept_move = move_dir(
        format!("upload/{}_processing", conceptid).as_str(),
        format!("source/{}", form.medium_id.to_ascii_lowercase()).as_str(),
    )
    .await;
    if concept_move.is_ok() {
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
        let description = form.medium_description.clone();
        let upload_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let medium_id_lower = form.medium_id.to_ascii_lowercase();

        // Insert into media table
        let _ = db.session.execute_unpaged(
            &db.insert_media,
            (&medium_id_lower, &form.medium_name, &description, &upload_ts, &user_info.login, &concept.r#type, &ispublic, &visibility, &restricted_to_group),
        ).await;

        // Insert into media_by_owner table
        let views = 0i64;
        let _ = db.session.execute_unpaged(
            &db.insert_media_by_owner,
            (&user_info.login, &upload_ts, &medium_id_lower, &form.medium_name, &description, &views, &concept.r#type, &ispublic, &visibility, &restricted_to_group),
        ).await;

        // Delete the concept from all tables
        let _ = db.session.execute_unpaged(&db.delete_concept, (&concept.id,)).await;
        let _ = db.session.execute_unpaged(&db.delete_concept_by_owner, (&user_info.login, &concept.id)).await;
        let _ = db.session.execute_unpaged(&db.delete_unprocessed_concept, (&concept.id,)).await;

        return Html(format!(
            "<script>window.location.replace(\"/m/{}\");</script>",
            form.medium_id
        ));
    } else {
        return Html(format!(
            "<h1>ERROR MOVING PROCESSED CONCEPT TO MEDIA!</h1><br>{:?}",
            concept_move
        ));
    }
}
