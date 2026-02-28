#[derive(Serialize, Deserialize, Clone)]
struct List {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ListWithCount {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
    item_count: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct ListModalEntry {
    id: String,
    name: String,
    already_added: bool,
}

#[derive(Deserialize)]
struct CreateListForm {
    name: String,
    visibility: Option<String>,
    restricted_group: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/list.html", escape = "none")]
struct ListPageTemplate {
    sidebar: String,
    config: Config,
    list: List,
    is_owner: bool,
}

#[derive(Template)]
#[template(path = "pages/hx-listitems.html", escape = "none")]
struct HXListItemsTemplate {
    items: Vec<Medium>,
    list_id: String,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}

#[derive(Template)]
#[template(path = "pages/hx-listmodal.html", escape = "none")]
struct HXListModalTemplate {
    lists: Vec<ListModalEntry>,
    medium_id: String,
    owner_groups: Vec<UserGroup>,
}

#[derive(Template)]
#[template(path = "pages/hx-userlists.html", escape = "none")]
struct HXUserListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
}

async fn list_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let list_row = sqlx::query(
        "SELECT id, name, owner, visibility, restricted_to_group FROM lists WHERE id=$1;"
    )
    .bind(&listid)
    .fetch_one(&pool)
    .await;

    let list = match list_row {
        Ok(row) => {
            use sqlx::Row;
            List {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
                visibility: row.get("visibility"),
                restricted_to_group: row.get("restricted_to_group"),
            }
        }
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    let is_owner = user_info
        .as_ref()
        .map(|u| u.login == list.owner)
        .unwrap_or(false);

    // Access control for restricted lists
    if !is_owner && !can_access_restricted(&pool, &list.visibility, list.restricted_to_group.as_deref(), &list.owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "list".to_owned());
    let template = ListPageTemplate {
        sidebar,
        config,
        list,
        is_owner,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn medium_in_list(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let list_row = sqlx::query(
        "SELECT id, visibility, restricted_to_group, owner, name FROM lists WHERE id=$1;"
    )
    .bind(&listid)
    .fetch_one(&pool)
    .await;

    let list = match list_row {
        Ok(row) => {
            use sqlx::Row;
            (
                row.get::<String, _>("id"),
                row.get::<String, _>("visibility"),
                row.get::<Option<String>, _>("restricted_to_group"),
                row.get::<String, _>("owner"),
                row.get::<String, _>("name"),
            )
        }
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    let is_logged_in = user_info.is_some();

    // Access control for restricted lists
    let is_owner = user_info.as_ref().map(|u| u.login == list.3).unwrap_or(false);
    if !is_owner && !can_access_restricted(&pool, &list.1, list.2.as_deref(), &list.3, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers).unwrap();

    // Also check media access
    let medium_row = sqlx::query(
        "SELECT id,name,description,upload,owner,views,type,visibility,restricted_to_group FROM media WHERE id=$1;"
    )
    .bind(mediumid.to_ascii_lowercase())
    .fetch_one(&pool)
    .await;

    let medium = match medium_row {
        Ok(row) => row,
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    use sqlx::Row;
    let m_visibility: String = medium.get("visibility");
    let m_restricted: Option<String> = medium.get("restricted_to_group");
    let m_owner: String = medium.get("owner");

    if !can_access_restricted(&pool, &m_visibility, m_restricted.as_deref(), &m_owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let medium_id: String = medium.get("id");
    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<String> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium_id)).exists() {
        medium_captions_exist = true;
        for caption_name in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium_id)) {
            medium_captions_list.push(caption_name);
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", medium_id)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium_id)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id,
        medium_name: medium.get("name"),
        medium_owner: medium.get("owner"),
        medium_upload: prettyunixtime(medium.get("upload")).await,
        medium_views: medium.get("views"),
        medium_type: medium.get("type"),
        medium_captions_exist,
        medium_captions_list,
        medium_chapters_exist,
        medium_previews_exist,
        config,
        common_headers,
        is_logged_in,
        list_id: listid,
        list_name: list.4,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_items(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, pool, listid, 0).await
}

async fn hx_list_items_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path((listid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, pool, listid, page).await
}

async fn hx_list_items_inner(
    config: Config,
    pool: PgPool,
    listid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let offset = page * 40;

    let mut items: Vec<Medium> = sqlx::query(
        "SELECT m.id, m.name, m.owner, m.views, m.type FROM list_items li INNER JOIN media m ON li.media_id = m.id WHERE li.list_id = $1 ORDER BY li.position ASC LIMIT 41 OFFSET $2;"
    )
    .bind(&listid)
    .bind(offset)
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

    let has_more = items.len() == 41;
    if has_more {
        items.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/listitems/{}/{}", listid, next_page);

    let template = HXListItemsTemplate {
        items,
        list_id: listid,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_sidebar(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let media: Vec<Medium> = sqlx::query_as!(
        Medium,
        "SELECT m.id, m.name, m.owner, m.views, m.type FROM list_items li INNER JOIN media m ON li.media_id = m.id WHERE li.list_id = $1 ORDER BY li.position ASC;",
        listid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXMediumListTemplate {
        media,
        current_medium_id: mediumid,
        config
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_modal(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let lists = sqlx::query_as!(
        ListModalEntry,
        "SELECT l.id, l.name, EXISTS(SELECT 1 FROM list_items li WHERE li.list_id = l.id AND li.media_id = $2) AS \"already_added!\" FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC;",
        user_info.login,
        mediumid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    // Fetch user's groups for visibility selection in create form
    let owner_groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
        .bind(&user_info.login)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        })
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_list(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CreateListForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list_id = generate_medium_id();
    let visibility = match form.visibility.as_deref() {
        Some("public") => "public",
        Some("restricted") => "restricted",
        _ => "hidden",
    };
    let restricted_to_group = if visibility == "restricted" {
        form.restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO lists (id, name, owner, visibility, restricted_to_group) VALUES ($1, $2, $3, $4, $5);"
    )
    .bind(&list_id)
    .bind(&form.name)
    .bind(&user_info.login)
    .bind(visibility)
    .bind(&restricted_to_group)
    .execute(&pool)
    .await
    .expect("Database error");

    sqlx::query!(
        "INSERT INTO list_items (list_id, media_id, position) VALUES ($1, $2, 0);",
        list_id,
        mediumid
    )
    .execute(&pool)
    .await
    .expect("Database error");

    let lists = sqlx::query_as!(
        ListModalEntry,
        "SELECT l.id, l.name, EXISTS(SELECT 1 FROM list_items li WHERE li.list_id = l.id AND li.media_id = $2) AS \"already_added!\" FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC;",
        user_info.login,
        mediumid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    // Fetch user's groups for visibility selection in create form
    let owner_groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
        .bind(&user_info.login)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        })
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_to_list(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list = sqlx::query!("SELECT owner FROM lists WHERE id=$1;", listid)
        .fetch_one(&pool)
        .await
        .expect("Database error");
    if list.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    let max_pos = sqlx::query!(
        "SELECT COALESCE(MAX(position), -1) AS max_pos FROM list_items WHERE list_id=$1;",
        listid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let next_pos = max_pos.max_pos.unwrap_or(-1) + 1;

    sqlx::query!(
        "INSERT INTO list_items (list_id, media_id, position) VALUES ($1, $2, $3);",
        listid,
        mediumid,
        next_pos
    )
    .execute(&pool)
    .await
    .expect("Database error");

    let lists = sqlx::query_as!(
        ListModalEntry,
        "SELECT l.id, l.name, EXISTS(SELECT 1 FROM list_items li WHERE li.list_id = l.id AND li.media_id = $2) AS \"already_added!\" FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC;",
        user_info.login,
        mediumid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let owner_groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
        .bind(&user_info.login)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        })
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_from_list(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list = sqlx::query!("SELECT owner FROM lists WHERE id=$1;", listid)
        .fetch_one(&pool)
        .await
        .expect("Database error");
    if list.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    sqlx::query!(
        "DELETE FROM list_items WHERE list_id=$1 AND media_id=$2;",
        listid,
        mediumid
    )
    .execute(&pool)
    .await
    .expect("Database error");

    let lists = sqlx::query_as!(
        ListModalEntry,
        "SELECT l.id, l.name, EXISTS(SELECT 1 FROM list_items li WHERE li.list_id = l.id AND li.media_id = $2) AS \"already_added!\" FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC;",
        user_info.login,
        mediumid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let owner_groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
        .bind(&user_info.login)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        })
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_list(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let list = sqlx::query!("SELECT owner FROM lists WHERE id=$1;", listid)
        .fetch_one(&pool)
        .await;

    match list {
        Ok(record) => {
            if record.owner != user_info.login {
                return Html(
                    "<script>window.location.replace(\"/\");</script>".to_owned(),
                );
            }
        }
        Err(_) => {
            return Html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            );
        }
    }

    let _ = sqlx::query!("DELETE FROM list_items WHERE list_id=$1;", listid)
        .execute(&pool)
        .await;

    let _ = sqlx::query!("DELETE FROM lists WHERE id=$1;", listid)
        .execute(&pool)
        .await;

    Html("<b class=\"text-success\">LIST DELETED</b><script>window.location.replace(\"/\");</script>".to_owned())
}

async fn hx_user_lists(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(pool, redis, headers, userid, 0).await
}

async fn hx_user_lists_page(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(pool, redis, headers, userid, page).await
}

async fn hx_user_lists_inner(
    pool: PgPool,
    redis: RedisConn,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let offset = page * 40;

    let mut lists: Vec<ListWithCount> = sqlx::query(
        "SELECT l.id, l.name, l.owner, l.visibility, l.restricted_to_group, (SELECT COUNT(*) FROM list_items li WHERE li.list_id = l.id) AS item_count FROM lists l WHERE l.owner = $1 AND (l.visibility = 'public' OR (l.visibility = 'restricted' AND l.restricted_to_group IN (SELECT group_id FROM user_group_members WHERE user_login = $2))) ORDER BY l.created DESC LIMIT 41 OFFSET $3;"
    )
    .bind(&userid)
    .bind(&user_login)
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
    let next_url = format!("/hx/userlists/{}/{}", userid, next_page);

    let template = HXUserListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
