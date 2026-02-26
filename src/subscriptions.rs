#[derive(Template)]
#[template(path = "pages/subscriptions.html", escape = "none")]
struct SubscriptionsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}

async fn subscriptions(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "subscribed".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SubscriptionsTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscriptions(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
) -> axum::response::Html<Vec<u8>> {
    let user = match get_user_login(headers, &pool, redis.clone()).await {
        Some(user) => user,
        None => {
            return Html(
                "Please log in to see your subscriptions"
                    .as_bytes()
                    .to_vec(),
            )
        }
    };

    let group_ids = get_user_group_ids(&pool, &user.login, redis.clone()).await;

    let media: Vec<Medium> = sqlx::query(
        "SELECT m.id, m.name, m.owner, m.views, m.type
         FROM media m
         INNER JOIN subscriptions s ON m.owner = s.target
         WHERE s.subscriber = $1 AND (m.visibility = 'public' OR (m.visibility = 'restricted' AND m.restricted_to_group = ANY($2)))
         ORDER BY m.upload DESC
         LIMIT 100;"
    )
    .bind(&user.login)
    .bind(&group_ids)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        Medium {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            views: row.get("views"),
            r#type: row.get("type"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXMediumCardTemplate { media, config };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscribe(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &pool, redis.clone()).await.unwrap();
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
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &pool, redis.clone()).await.unwrap();
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
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, redis.clone()).await {
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
                userid
            )
        } else {
            format!(
                "<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",
                userid
            )
        };

        return Html(button);
    }

    Html("<a href=\"/login\" class=\"btn btn-primary\" preload=\"mouseover\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>".to_string())
}
