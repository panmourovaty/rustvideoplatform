#[derive(Template)]
#[template(path = "pages/subscriptions.html")]
struct SubscriptionsTemplate {
    sidebar: String,
    config: Config,
    current_user: Option<User>,
    locale: RequestLocale,
    resolved_lang: String,
}

async fn subscriptions(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let current_user = get_user_login(headers, &db, redis).await;
    let sidebar = generate_sidebar(
        &config,
        "subscribed".to_owned(),
        current_user.clone(),
        locale.clone(),
    );
    let template = SubscriptionsTemplate {
        sidebar,
        config,
        current_user,
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscriptions(
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
) -> axum::response::Html<Vec<u8>> {
    hx_subscriptions_inner(config, localization, headers, db, redis, 0).await
}

async fn hx_subscriptions_page(
    Extension(config): Extension<Config>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_subscriptions_inner(config, localization, headers, db, redis, page).await
}

async fn hx_subscriptions_inner(
    config: Config,
    localization: Arc<LocalizationService>,
    headers: HeaderMap,
    db: ScyllaDb,
    redis: RedisConn,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    if !valid_page(page) {
        return Html(Vec::new());
    }
    let common_headers = extract_common_headers(&headers);
    let user = match get_user_login(headers, &db, redis.clone()).await {
        Some(user) => user,
        None => {
            return Html(
                "Please log in to see your subscriptions"
                    .as_bytes()
                    .to_vec(),
            )
        }
    };

    let offset = (page * 30) as usize;

    // Get all subscribed channels
    let targets: Vec<String> = db.session.execute_unpaged(&db.get_subscriptions, (&user.login,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).map(|r| r.0).collect())
        .unwrap_or_default();

    // Fan out: get media from each subscribed channel (with owner tracking)
    struct MediaWithOwner {
        id: String,
        name: String,
        owner: String,
        views: i64,
        media_type: String,
        upload: i64,
        visibility: String,
        restricted_to_group: Option<String>,
    }

    let mut all_media_owned: Vec<MediaWithOwner> = Vec::new();
    for target in &targets {
        let media_rows = db.session.execute_unpaged(&db.get_media_by_owner, (target, 100i32))
            .await.ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>()
                .unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
            .unwrap_or_default();
        for row in media_rows {
            all_media_owned.push(MediaWithOwner {
                id: row.0,
                name: row.1,
                owner: target.clone(),
                views: row.3,
                media_type: row.4,
                upload: row.5,
                visibility: row.6,
                restricted_to_group: row.7,
            });
        }
    }

    // Sort by upload DESC
    all_media_owned.sort_by_key(|medium| std::cmp::Reverse(medium.upload));

    // Filter by visibility and paginate
    let user_opt = Some(user.clone());
    let mut media: Vec<Medium> = Vec::new();
    let mut skipped = 0usize;
    let mut taken = 0usize;
    for item in &all_media_owned {
        if !can_access_restricted(
            &db,
            &item.visibility,
            item.restricted_to_group.as_deref(),
            &item.owner,
            &user_opt,
            redis.clone(),
        ).await {
            continue;
        }

        if skipped < offset {
            skipped += 1;
            continue;
        }

        if taken >= 31 {
            break;
        }

        media.push(Medium {
            id: item.id.clone(),
            name: item.name.clone(),
            owner: item.owner.clone(),
            views: item.views,
            r#type: item.media_type.clone(),
            sprite_filename: None,
            sprite_x: 0,
            sprite_y: 0,
            visit_time: None,
        });
        taken += 1;
    }

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/subscriptions/{}", next_page);

    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscribe(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let Some(user) = get_user_login(headers, &db, redis.clone()).await else {
        return Html(String::new());
    };
    let target_exists = db
        .session
        .execute_unpaged(&db.check_user_exists, (&userid,))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .is_some();
    if !target_exists || userid == user.login {
        return Html(String::new());
    }
    if db
        .session
        .execute_unpaged(&db.insert_subscription, (&user.login, &userid))
        .await
        .is_err()
    {
        return Html(String::new());
    }
    if db
        .session
        .execute_unpaged(&db.insert_subscriber_by_target, (&userid, &user.login))
        .await
        .is_err()
    {
        let _ = db
            .session
            .execute_unpaged(&db.delete_subscription, (&user.login, &userid))
            .await;
        return Html(String::new());
    }
    Html(format!("<a hx-post=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>", escape_html(&userid)))
}
async fn hx_unsubscribe(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let Some(user) = get_user_login(headers, &db, redis.clone()).await else {
        return Html(String::new());
    };
    if db
        .session
        .execute_unpaged(&db.delete_subscription, (&user.login, &userid))
        .await
        .is_err()
    {
        return Html(String::new());
    }
    if db
        .session
        .execute_unpaged(&db.delete_subscriber_by_target, (&userid, &user.login))
        .await
        .is_err()
    {
        let _ = db
            .session
            .execute_unpaged(&db.insert_subscription, (&user.login, &userid))
            .await;
        return Html(String::new());
    }
    Html(format!("<a hx-post=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>", escape_html(&userid)))
}
async fn hx_subscribebutton(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let issubscribed = db.session.execute_unpaged(&db.is_subscribed, (&user.login, &userid))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.maybe_first_row::<(String,)>().ok().flatten().is_some())
            .unwrap_or(false);

        let button = if issubscribed {
            format!(
                "<a hx-post=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>",
                escape_html(&userid)
            )
        } else {
            format!(
                "<a hx-post=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",
                escape_html(&userid)
            )
        };

        return Html(button);
    }

    Html("<a href=\"/login\" class=\"btn btn-primary\" preload=\"mouseover\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>".to_string())
}
