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

#[derive(Template)]
#[template(path = "pages/hx-usermedia.html", escape = "none")]
struct HXUserMediaTemplate {
    usermedia: Vec<Medium>,
}
async fn hx_usermedia(
    Extension(pool): Extension<PgPool>,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let usermedia = sqlx::query_as!(Medium,
        "SELECT id,name,owner,views,type FROM media WHERE public=true AND owner=$1 ORDER BY upload DESC;",userid
    )
    .fetch_all(&pool)
    .await
    .expect("Nemohu provést dotaz");
    let template = HXUserMediaTemplate { usermedia };
    Html(minifi_html(template.render().unwrap()))
}
