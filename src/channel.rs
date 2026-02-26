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
}
async fn channel(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = sqlx::query_as!(
        UserChannel,
        "SELECT
    u.login,
    u.name,
    u.profile_picture,
    u.channel_picture,
    COALESCE(subs.count, 0) AS subscribed
FROM
    users u
LEFT JOIN
    (
        SELECT
            target,
            COUNT(*) AS count
        FROM
            subscriptions
        GROUP BY
            target
    ) subs
ON
    u.login = subs.target
WHERE
    u.login = $1;",
        userid
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sidebar = generate_sidebar(&config, "channel".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ChannelTemplate {
        sidebar,
        config,
        common_headers,
        user,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_usermedia(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let group_ids = get_user_group_ids(&pool, &user_login, redis.clone()).await;

    let media: Vec<Medium> = sqlx::query(
        "SELECT id,name,owner,views,type FROM media WHERE owner=$1 AND (visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group = ANY($2))) ORDER BY upload DESC;"
    )
    .bind(&userid)
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
