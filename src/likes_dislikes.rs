fn like_dislike_html(mediumid: &str, likes: i64, dislikes: i64, user_reaction: Option<&str>) -> String {
    let mediumid = escape_html(mediumid);
    let like_color = if user_reaction == Some("like") { "var(--bs-success)" } else { "var(--bs-white)" };
    let dislike_color = if user_reaction == Some("dislike") { "var(--bs-danger)" } else { "var(--bs-white)" };
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" style="cursor: pointer; color: {like_color}" hx-post="/hx/like/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" style="cursor: pointer; color: {dislike_color}" hx-post="/hx/dislike/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

fn like_dislike_html_unauth(likes: i64, dislikes: i64) -> String {
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" href="/login"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" href="/login"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

/// Get reaction counts from DB (authoritative source). Used after write operations.
async fn get_reaction_state_db(db: &ScyllaDb, mediumid: &str, user_login: Option<&str>) -> (i64, i64, Option<String>) {
    // Get all reactions for this media and count likes/dislikes in application code
    let reactions: Vec<(String, String)> = db.session.execute_unpaged(&db.get_reactions_for_media, (&mediumid,))
        .await.ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String)>().unwrap().filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let likes = reactions.iter().filter(|r| r.1 == "like").count() as i64;
    let dislikes = reactions.iter().filter(|r| r.1 == "dislike").count() as i64;

    // Get user's personal reaction
    let user_reaction = if let Some(login) = user_login {
        db.session.execute_unpaged(&db.get_user_reaction, (&mediumid, &login))
            .await.ok().and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
            .map(|r| r.0)
    } else {
        None
    };

    (likes, dislikes, user_reaction)
}

async fn get_user_reaction_db(
    db: &ScyllaDb,
    medium_id: &str,
    user_login: &str,
) -> Option<String> {
    db.session
        .execute_unpaged(&db.get_user_reaction, (medium_id, user_login))
        .await
        .ok()
        .and_then(|result| result.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|row| row.0)
}

/// Get reaction state using Redis cache for counts (falls back to DB on cache miss).
/// User's personal reaction is always fetched from DB.
async fn get_reaction_state_cached(db: &ScyllaDb, mediumid: &str, user_login: Option<&str>, mut redis: RedisConn) -> (i64, i64, Option<String>) {
    // Try getting counts from Redis cache
    let cached_likes: Result<i64, _> = redis.get(format!("cache:media:{}:likes", mediumid)).await;
    let cached_dislikes: Result<i64, _> = redis.get(format!("cache:media:{}:dislikes", mediumid)).await;

    let (likes, dislikes) = if let (Ok(l), Ok(d)) = (cached_likes, cached_dislikes) {
        (l, d)
    } else {
        // Cache miss — fall back to DB
        let reactions: Vec<(String, String)> = db.session.execute_unpaged(&db.get_reactions_for_media, (&mediumid,))
            .await.ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String, String)>().unwrap().filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        let l = reactions.iter().filter(|r| r.1 == "like").count() as i64;
        let d = reactions.iter().filter(|r| r.1 == "dislike").count() as i64;
        let _: Result<(), _> = redis.set(format!("cache:media:{}:likes", mediumid), l).await;
        let _: Result<(), _> = redis.set(format!("cache:media:{}:dislikes", mediumid), d).await;
        (l, d)
    };

    // User's personal reaction must come from DB (per-user, not worth caching individually)
    let user_reaction = if let Some(login) = user_login {
        db.session.execute_unpaged(&db.get_user_reaction, (&mediumid, &login))
            .await.ok().and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
            .map(|r| r.0)
    } else {
        None
    };

    (likes, dislikes, user_reaction)
}

/// Update the cached reaction counts in Redis after a write operation.
async fn update_reaction_cache(redis: &mut RedisConn, mediumid: &str, likes: i64, dislikes: i64) {
    let _: Result<(), _> = redis.set(format!("cache:media:{}:likes", mediumid), likes).await;
    let _: Result<(), _> = redis.set(format!("cache:media:{}:dislikes", mediumid), dislikes).await;
}

async fn apply_reaction_cache_delta(
    redis: &mut RedisConn,
    medium_id: &str,
    like_delta: i64,
    dislike_delta: i64,
) -> Option<(i64, i64)> {
    let likes: i64 = redis
        .incr(format!("cache:media:{medium_id}:likes"), like_delta)
        .await
        .ok()?;
    let dislikes: i64 = redis
        .incr(
            format!("cache:media:{medium_id}:dislikes"),
            dislike_delta,
        )
        .await
        .ok()?;
    Some((likes.max(0), dislikes.max(0)))
}

async fn hx_likedislikebutton(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if !can_access_media_request(&headers, &db, redis.clone(), &mediumid).await {
        return Html(String::new());
    }
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let (likes, dislikes, user_reaction) = get_reaction_state_cached(&db, &mediumid, Some(&user.login), redis.clone()).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(likes, dislikes))
    }
}

async fn hx_like(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if !can_access_media_request(&headers, &db, redis.clone(), &mediumid).await {
        return Html(String::new());
    }
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let _ = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        let current_reaction = get_user_reaction_db(&db, &mediumid, &user.login).await;

        let (write_result, like_delta, dislike_delta, next_reaction) =
            if current_reaction.as_deref() == Some("like") {
                (
                    db.session
                        .execute_unpaged(&db.delete_reaction, (&mediumid, &user.login))
                        .await,
                    -1,
                    0,
                    None,
                )
            } else {
                (
                    db.session
                        .execute_unpaged(&db.upsert_reaction, (&mediumid, &user.login, &"like"))
                        .await,
                    1,
                    -i64::from(current_reaction.as_deref() == Some("dislike")),
                    Some("like".to_owned()),
                )
            };
        if write_result.is_err() {
            return Html(String::new());
        }

        let mut cache_redis = redis.clone();
        if let Some((likes, dislikes)) = apply_reaction_cache_delta(
            &mut cache_redis,
            &mediumid,
            like_delta,
            dislike_delta,
        )
        .await
        {
            Html(like_dislike_html(
                &mediumid,
                likes,
                dislikes,
                next_reaction.as_deref(),
            ))
        } else {
            let (likes, dislikes, reaction) =
                get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;
            update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
            Html(like_dislike_html(
                &mediumid,
                likes,
                dislikes,
                reaction.as_deref(),
            ))
        }
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(likes, dislikes))
    }
}

async fn hx_dislike(
    headers: HeaderMap,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if !can_access_media_request(&headers, &db, redis.clone(), &mediumid).await {
        return Html(String::new());
    }
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let _ = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        let current_reaction = get_user_reaction_db(&db, &mediumid, &user.login).await;

        let (write_result, like_delta, dislike_delta, next_reaction) =
            if current_reaction.as_deref() == Some("dislike") {
                (
                    db.session
                        .execute_unpaged(&db.delete_reaction, (&mediumid, &user.login))
                        .await,
                    0,
                    -1,
                    None,
                )
            } else {
                (
                    db.session
                        .execute_unpaged(&db.upsert_reaction, (&mediumid, &user.login, &"dislike"))
                        .await,
                    -i64::from(current_reaction.as_deref() == Some("like")),
                    1,
                    Some("dislike".to_owned()),
                )
            };
        if write_result.is_err() {
            return Html(String::new());
        }

        let mut cache_redis = redis.clone();
        if let Some((likes, dislikes)) = apply_reaction_cache_delta(
            &mut cache_redis,
            &mediumid,
            like_delta,
            dislike_delta,
        )
        .await
        {
            Html(like_dislike_html(
                &mediumid,
                likes,
                dislikes,
                next_reaction.as_deref(),
            ))
        } else {
            let (likes, dislikes, reaction) =
                get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;
            update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
            Html(like_dislike_html(
                &mediumid,
                likes,
                dislikes,
                reaction.as_deref(),
            ))
        }
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(likes, dislikes))
    }
}
