async fn hx_subscribe(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &pool, session_store).await.unwrap();
    sqlx::query!(
        "INSERT INTO subscriptions (subscriber, target) VALUES ($1,$2);",
        user.login,
        userid
    )
    .execute(&pool)
    .await
    .expect("Database error");
    Html(format!("<a hx-get=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>",user.login))
}
async fn hx_unsubscribe(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &pool, session_store).await.unwrap();
    sqlx::query!(
        "DELETE FROM subscriptions WHERE subscriber=$1 AND target=$2;",
        user.login,
        userid
    )
    .execute(&pool)
    .await
    .expect("Database error");
    Html(format!("<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",user.login))
}
async fn hx_subscribebutton(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, session_store).await {
        let issubscribed = sqlx::query!(
            "SELECT EXISTS(SELECT FROM subscriptions WHERE subscriber=$1 AND target=$2) AS issubscribed;",
            user.login,
            userid
        )
        .fetch_one(&pool)
        .await
        .map(|row| row.issubscribed.unwrap_or(false))
        .unwrap_or(false);

        let button = if issubscribed {
            format!(
                "<a hx-get=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>",
                user.login
            )
        } else {
            format!(
                "<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",
                user.login
            )
        };

        return Html(button);
    }

    Html("<a href=\"/login\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>".to_string())
}