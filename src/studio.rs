#[derive(Template)]
#[template(path = "pages/studio.html", escape = "none")]
struct StudioTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    active_tab: String,
}
async fn studio(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "media".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumStudio {
    id: String,
    name: String,
    description: Option<serde_json::Value>,
    views: i64,
    r#type: String,
}
#[derive(Template)]
#[template(path = "pages/hx-studio.html", escape = "none")]
struct HXStudioTemplate {
    media: Vec<MediumStudio>,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, pool, redis, headers, 0).await
}

async fn hx_studio_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, pool, redis, headers, page).await
}

async fn hx_studio_inner(
    config: Config,
    pool: PgPool,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let offset = page * 40;

    let mut media: Vec<MediumStudio> = sqlx::query(
        "SELECT id,name,description,views,type FROM media WHERE owner=$1 ORDER BY upload DESC LIMIT 41 OFFSET $2;"
    )
    .bind(&user_info.login)
    .bind(offset)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        MediumStudio {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            views: row.get("views"),
            r#type: row.get("type"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/studio/{}", next_page);

    let template = HXStudioTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

async fn studio_lists(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "lists".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-lists.html", escape = "none")]
struct HXStudioListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio_lists(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(pool, redis, headers, 0).await
}

async fn hx_studio_lists_page(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(pool, redis, headers, page).await
}

async fn hx_studio_lists_inner(
    pool: PgPool,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let offset = page * 40;

    let mut lists: Vec<ListWithCount> = sqlx::query(
        "SELECT l.id, l.name, l.owner, l.visibility, l.restricted_to_group, (SELECT COUNT(*) FROM list_items li WHERE li.list_id = l.id) AS item_count FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC LIMIT 41 OFFSET $2;"
    )
    .bind(&user_info.login)
    .bind(offset)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        ListWithCount {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            visibility: row.get("visibility"),
            restricted_to_group: row.get("restricted_to_group"),
            item_count: row.get("item_count"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let has_more = lists.len() == 41;
    if has_more {
        lists.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/studio/lists/{}", next_page);

    let template = HXStudioListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumEdit {
    id: String,
    name: String,
    visibility: String,
    restricted_to_group: String,
    medium_type: String,
}
#[derive(Template)]
#[template(path = "pages/studio-edit.html", escape = "none")]
struct StudioEditTemplate {
    sidebar: String,
    config: Config,
    medium: MediumEdit,
    common_headers: CommonHeaders,
    owner_groups: Vec<UserGroup>,
}
async fn studio_edit(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let medium = sqlx::query(
        "SELECT id,name,owner,visibility,restricted_to_group,type FROM media WHERE id=$1;"
    )
    .bind(&mediumid)
    .fetch_one(&pool)
    .await;

    match medium {
        Ok(record) => {
            use sqlx::Row;
            let owner: String = record.get("owner");
            if owner != user_info.login {
                return Html(minifi_html(
                    "<script>window.location.replace(\"/studio\");</script>".to_owned(),
                ));
            }

            // Fetch user's groups for the dropdown
            let owner_groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
                .bind(&user_info.login)
                .map(|row: sqlx::postgres::PgRow| {
                    UserGroup {
                        id: row.get("id"),
                        name: row.get("name"),
                        owner: row.get("owner"),
                    }
                })
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

            let sidebar = generate_sidebar(&config, "studio".to_owned());
            let common_headers = extract_common_headers(&headers).unwrap();
            let template = StudioEditTemplate {
                sidebar,
                config,
                medium: MediumEdit {
                    id: record.get("id"),
                    name: record.get("name"),
                    visibility: record.get("visibility"),
                    restricted_to_group: record.get::<Option<String>, _>("restricted_to_group").unwrap_or_default(),
                    medium_type: record.get("type"),
                },
                common_headers,
                owner_groups,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        Err(_) => Html(minifi_html(
            "<script>window.location.replace(\"/studio\");</script>".to_owned(),
        )),
    }
}

#[derive(Serialize, Deserialize)]
struct EditForm {
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}
async fn studio_edit_save(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<EditForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
            }
        }
        Err(_) => {
            return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
        }
    }

    let visibility = match form.medium_visibility.as_str() {
        "public" | "hidden" | "restricted" => form.medium_visibility.clone(),
        _ => "hidden".to_owned(),
    };
    let restricted_to_group = if visibility == "restricted" {
        form.medium_restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };
    let description: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or(serde_json::Value::Null);

    let update_result = sqlx::query(
        "UPDATE media SET name=$1, description=$2, visibility=$3, restricted_to_group=$4 WHERE id=$5;"
    )
    .bind(&form.medium_name)
    .bind(&description)
    .bind(&visibility)
    .bind(&restricted_to_group)
    .bind(&mediumid)
    .execute(&pool)
    .await;

    if update_result.is_err() {
        return Html(format!(
            "<b class=\"text-danger\">Failed to save changes.</b><script>setTimeout(function(){{window.location.replace(\"/studio/edit/{}\");}},2000);</script>",
            mediumid
        ));
    }

    Html(format!(
        "<script>window.location.replace(\"/studio/edit/{}\");</script>",
        mediumid
    ))
}

async fn hx_delete_video(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return ([("HX-Redirect", "/login")], "");
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return ([("HX-Redirect", "/studio")], "");
            }
        }
        Err(_) => {
            return ([("HX-Redirect", "/studio")], "");
        }
    }

    // Delete associated comments first
    let _ = sqlx::query!("DELETE FROM comments WHERE media=$1;", mediumid)
        .execute(&pool)
        .await;

    // Delete from any lists
    let _ = sqlx::query!("DELETE FROM list_items WHERE media_id=$1;", mediumid)
        .execute(&pool)
        .await;

    // Delete the video from database
    let delete_result = sqlx::query!("DELETE FROM media WHERE id=$1;", mediumid)
        .execute(&pool)
        .await;

    if delete_result.is_err() {
        return ([("Redirect", "/studio")], "");
    }

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    ([("Redirect", "/studio")], "")
}
