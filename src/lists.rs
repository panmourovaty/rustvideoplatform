#[derive(Serialize, Deserialize, Clone)]
struct List {
    id: String,
    name: String,
    owner: String,
    public: bool,
}

#[derive(Serialize, Deserialize)]
struct ListWithCount {
    id: String,
    name: String,
    owner: String,
    public: bool,
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
    public: Option<String>,
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
}

#[derive(Template)]
#[template(path = "pages/hx-listmodal.html", escape = "none")]
struct HXListModalTemplate {
    lists: Vec<ListModalEntry>,
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-userlists.html", escape = "none")]
struct HXUserListsTemplate {
    lists: Vec<ListWithCount>,
}

async fn list_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let list = sqlx::query_as!(
        List,
        "SELECT id, name, owner, public FROM lists WHERE id=$1;",
        listid
    )
    .fetch_one(&pool)
    .await;

    let list = match list {
        Ok(l) => l,
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    let is_owner = user_info
        .as_ref()
        .map(|u| u.login == list.owner)
        .unwrap_or(false);

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
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let list = sqlx::query!(
        "SELECT id, public, owner, name FROM lists WHERE id=$1;",
        listid
    )
    .fetch_one(&pool)
    .await;

    let list = match list {
        Ok(l) => l,
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    let is_logged_in = user_info.is_some();

    let common_headers = extract_common_headers(&headers).unwrap();
    let medium = sqlx::query!(
        "SELECT id,name,description,upload,owner,likes,dislikes,views,type FROM media WHERE id=$1;",
        mediumid.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<String> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium.id)).exists() {
        medium_captions_exist = true;
        for caption_name in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium.id)) {
            medium_captions_list.push(caption_name);
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", medium.id)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium.id)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let medium_document_filename = match medium.r#type.as_str() {
        "document_writer" => "document.odt".to_owned(),
        "document_spreadsheet" => "document.ods".to_owned(),
        "document_presentation" => "document.odp".to_owned(),
        _ => String::new(),
    };

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id: medium.id,
        medium_name: medium.name,
        medium_owner: medium.owner,
        medium_likes: medium.likes,
        medium_dislikes: medium.dislikes,
        medium_upload: prettyunixtime(medium.upload).await,
        medium_views: medium.views,
        medium_type: medium.r#type,
        medium_captions_exist,
        medium_captions_list,
        medium_chapters_exist,
        medium_previews_exist,
        medium_document_filename,
        config,
        common_headers,
        is_logged_in,
        list_id: listid,
        list_name: list.name,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_items(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let items: Vec<Medium> = sqlx::query_as!(
        Medium,
        "SELECT m.id, m.name, m.owner, m.views, m.type FROM list_items li INNER JOIN media m ON li.media_id = m.id WHERE li.list_id = $1 ORDER BY li.position ASC;",
        listid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXListItemsTemplate {
        items,
        list_id: listid,
        config
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
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
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

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_list(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CreateListForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list_id = generate_medium_id();
    let is_public = form.public.is_some();

    sqlx::query!(
        "INSERT INTO lists (id, name, owner, public) VALUES ($1, $2, $3, $4);",
        list_id,
        form.name,
        user_info.login,
        is_public
    )
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

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_to_list(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
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

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_from_list(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
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

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_list(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
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
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let lists = sqlx::query_as!(
        ListWithCount,
        "SELECT l.id, l.name, l.owner, l.public, (SELECT COUNT(*) FROM list_items li WHERE li.list_id = l.id) AS item_count FROM lists l WHERE l.owner = $1 AND l.public = true ORDER BY l.created DESC;",
        userid
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXUserListsTemplate { lists };
    Html(minifi_html(template.render().unwrap()))
}
