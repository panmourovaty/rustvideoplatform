#[derive(Serialize, Deserialize)]
struct Comment {
    id: i64,
    user: String,
    user_name: String,
    user_picture: Option<String>,
    text: serde_json::Value,
    time: i64,
}

#[derive(Deserialize)]
struct CommentsQuery {
    page: Option<i64>,
}

#[derive(Template)]
#[template(path = "pages/hx-comments.html", escape = "none")]
struct HXCommentsTemplate {
    comments: Vec<Comment>,
    medium_id: String,
    next_page: Option<i64>,
    config: Config,
    locale: RequestLocale,
}

const COMMENTS_PER_PAGE: i64 = 20;

async fn hx_comments(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    axum::extract::Query(query): axum::extract::Query<CommentsQuery>,
) -> axum::response::Html<Vec<u8>> {
    let page = query.page.unwrap_or(0);
    // ScyllaDB doesn't support OFFSET, so fetch enough rows to cover page+1 pages and skip in app code.
    let fetch_limit = (page + 1) * COMMENTS_PER_PAGE + 1;

    let rows = db.session.execute_unpaged(&db.get_comments, (&mediumid, fetch_limit as i32))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(i64, String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    // Skip rows for previous pages
    let skip = (page * COMMENTS_PER_PAGE) as usize;
    let page_rows: Vec<_> = rows.into_iter().skip(skip).take((COMMENTS_PER_PAGE + 1) as usize).collect();

    let has_more = page_rows.len() as i64 > COMMENTS_PER_PAGE;

    // Batch-fetch user info for each commenter
    let mut comments = Vec::new();
    for (id, user, text, time) in page_rows.into_iter().take(COMMENTS_PER_PAGE as usize) {
        let (user_name, user_picture) = db.session.execute_unpaged(&db.get_user_by_login, (&user,))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .and_then(|rows| rows.maybe_first_row::<(String, Option<String>)>().ok().flatten())
            .unwrap_or_else(|| (user.clone(), None));

        let text_value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();

        comments.push(Comment {
            id,
            user,
            user_name,
            user_picture,
            text: text_value,
            time,
        });
    }

    let next_page = if has_more { Some(page + 1) } else { None };

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXCommentsTemplate {
        comments,
        medium_id: mediumid,
        next_page,
        config,
        locale,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Deserialize)]
struct CommentForm {
    text: String,
}

async fn comment_delta(
    Extension(_db): Extension<ScyllaDb>,
    Path(_comment_id): Path<i64>,
) -> Json<serde_json::Value> {
    // Cannot look up by comment ID alone; the comments table has PRIMARY KEY (media, time, id).
    // Returning null as a fallback until the route is refactored to include media + time params.
    Json(serde_json::Value::Null)
}

#[derive(Template)]
#[template(path = "pages/hx-comment-single.html", escape = "none")]
struct HXCommentSingleTemplate {
    comment: Comment,
    config: Config,
}

async fn hx_add_comment(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CommentForm>,
) -> impl IntoResponse {
    let user = get_user_login(headers, &db, redis.clone()).await;

    if user.is_none() {
        return (StatusCode::UNAUTHORIZED, Html(Vec::new()));
    }

    let user = user.unwrap();

    let delta: serde_json::Value =
        serde_json::from_str(&form.text).unwrap_or_default();
    let delta_string = serde_json::to_string(&delta).unwrap_or_default();

    let comment_id = generate_comment_id();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    // Insert comment into ScyllaDB
    let _ = db.session.execute_unpaged(
        &db.insert_comment,
        (&mediumid, now, comment_id, &user.login, &delta_string),
    )
    .await;

    // Fetch user info separately
    let (user_name, user_picture) = db.session.execute_unpaged(&db.get_user_by_login, (&user.login,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, Option<String>)>().ok().flatten())
        .unwrap_or_else(|| (user.login.clone(), None));

    let comment = Comment {
        id: comment_id,
        user: user.login,
        user_name,
        user_picture,
        text: delta,
        time: now,
    };

    let template = HXCommentSingleTemplate { comment, config };
    (StatusCode::OK, Html(minifi_html(template.render().unwrap())))
}
