#[derive(Serialize, Deserialize)]
struct UserChannel {
    login: String,
    name: String,
    profile_picture: Option<String>,
    channel_picture: Option<String>,
    subscribed: Option<i64>,
}
#[derive(Template)]
#[template(path = "pages/channel.html", escape = "none")]
struct ChannelTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    user: UserChannel,
    schema_org_json: String,
    locale: RequestLocale,
    resolved_lang: String,
}
async fn channel(
    Extension(db): Extension<ScyllaDb>,
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    // Get user name + profile_picture from users table
    let user_info = db.session.execute_unpaged(&db.get_user_by_login, (&userid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>, Option<String>)>().ok().flatten());

    let (name, profile_picture) = match user_info {
        Some((name_opt, pic)) => (name_opt.unwrap_or_default(), pic),
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    // Get channel_picture from users table
    let channel_picture = db.session.execute_unpaged(&db.get_user_channel_picture, (&userid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(Option<String>,)>().ok().flatten())
        .map(|row| row.0)
        .unwrap_or(None);

    // Count subscribers
    let subscriber_rows = db.session.execute_unpaged(&db.count_subscribers, (&userid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();
    let subscriber_count = subscriber_rows.len() as i64;

    let user = UserChannel {
        login: userid,
        name,
        profile_picture,
        channel_picture,
        subscribed: Some(subscriber_count),
    };

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "channel".to_owned(), locale.clone());
    let schema_org_json = {
        let profile_url = format!("{}/u/{}", config.site_url, user.login);
        let mut main_entity = serde_json::json!({
            "@type": "Person",
            "name": &user.name,
            "identifier": &user.login,
            "url": &profile_url,
            "interactionStatistic": {
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/FollowAction",
                "userInteractionCount": user.subscribed.unwrap_or(0)
            }
        });
        if let Some(ref pic) = user.channel_picture {
            main_entity["image"] = serde_json::Value::String(
                format!("{}/source/{}/picture.avif", config.source_server_url, pic)
            );
        }
        serde_json::to_string(&serde_json::json!({
            "@context": "https://schema.org",
            "@type": "ProfilePage",
            "name": format!("{} - {}", user.name, config.instancename),
            "url": &profile_url,
            "mainEntity": main_entity
        })).unwrap_or_default()
    };
    let template = ChannelTemplate {
        sidebar,
        config,
        common_headers,
        user,
        schema_org_json,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_usermedia(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, db, redis, headers, userid, 0).await
}

async fn hx_usermedia_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, db, redis, headers, userid, page).await
}

async fn hx_usermedia_inner(
    config: Config,
    db: ScyllaDb,
    redis: RedisConn,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    let offset = (page * 40) as usize;

    // Fetch more than needed to handle app-level filtering and pagination.
    // No OFFSET in Cassandra, so we fetch a larger batch and skip in app code.
    let fetch_limit = (offset + 41) as i32;

    let all_rows = db.session.execute_unpaged(&db.get_media_by_owner, (&userid, fetch_limit)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>()
            .unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Filter visibility at application level (no subqueries in Cassandra)
    let mut filtered = Vec::new();
    for row in all_rows {
        let visibility = row.6.as_str();
        let restricted_to_group = row.7.as_deref();
        if can_access_restricted(&db, visibility, restricted_to_group, &userid, &user, redis.clone()).await {
            filtered.push(Medium {
                id: row.0,
                name: row.1,
                owner: userid.clone(),
                views: row.3,
                r#type: row.4,
                sprite_filename: None,
                sprite_x: 0,
                sprite_y: 0,
                visit_time: None,
            });
        }
    }

    // Paginate in app code: skip `offset` items, take up to 41 for has_more check
    let paginated: Vec<Medium> = filtered.into_iter().skip(offset).take(41).collect();

    let mut media = paginated;
    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/usermedia/{}/{}", userid, next_page);

    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}
