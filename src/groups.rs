#[derive(Serialize, Deserialize, Clone)]
struct UserGroup {
    id: String,
    name: String,
    owner: String,
}

#[derive(Serialize, Deserialize)]
struct UserGroupWithCount {
    id: String,
    name: String,
    owner: String,
    member_count: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct GroupMember {
    user_login: String,
}

#[derive(Deserialize)]
struct CreateGroupForm {
    name: String,
}

#[derive(Deserialize)]
struct AddMemberForm {
    user_login: String,
}

async fn studio_groups(
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
        active_tab: "groups".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-groups.html", escape = "none")]
struct HXStudioGroupsTemplate {
    groups: Vec<UserGroupWithCount>,
}

async fn hx_studio_groups(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let groups: Vec<UserGroupWithCount> = sqlx::query(
        "SELECT g.id, g.name, g.owner, COUNT(gm.group_id) AS member_count FROM user_groups g LEFT JOIN user_group_members gm ON g.id = gm.group_id WHERE g.owner = $1 GROUP BY g.id, g.name, g.owner ORDER BY g.created DESC;"
    )
    .bind(&user_info.login)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        UserGroupWithCount {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            member_count: row.get("member_count"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_group(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<CreateGroupForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let group_id = generate_medium_id();

    sqlx::query("INSERT INTO user_groups (id, name, owner) VALUES ($1, $2, $3);")
        .bind(&group_id)
        .bind(&form.name)
        .bind(&user_info.login)
        .execute(&pool)
        .await
        .expect("Database error");

    // Return updated groups list
    let groups: Vec<UserGroupWithCount> = sqlx::query(
        "SELECT g.id, g.name, g.owner, COUNT(gm.group_id) AS member_count FROM user_groups g LEFT JOIN user_group_members gm ON g.id = gm.group_id WHERE g.owner = $1 GROUP BY g.id, g.name, g.owner ORDER BY g.created DESC;"
    )
    .bind(&user_info.login)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        UserGroupWithCount {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            member_count: row.get("member_count"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_group(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    let owner_check: Option<sqlx::postgres::PgRow> = sqlx::query("SELECT owner FROM user_groups WHERE id = $1;")
        .bind(&groupid)
        .fetch_optional(&pool)
        .await
        .expect("Database error");

    if let Some(row) = owner_check {
        use sqlx::Row;
        let owner: String = row.get("owner");
        if owner != user_info.login {
            return Html("".as_bytes().to_vec());
        }
    } else {
        return Html("".as_bytes().to_vec());
    }

    // Clear group references from media and lists
    sqlx::query("UPDATE media SET visibility = 'hidden', restricted_to_group = NULL WHERE restricted_to_group = $1;")
        .bind(&groupid)
        .execute(&pool)
        .await
        .expect("Database error");

    sqlx::query("UPDATE lists SET visibility = 'hidden', restricted_to_group = NULL WHERE restricted_to_group = $1;")
        .bind(&groupid)
        .execute(&pool)
        .await
        .expect("Database error");

    // Delete members and group
    sqlx::query("DELETE FROM user_group_members WHERE group_id = $1;")
        .bind(&groupid)
        .execute(&pool)
        .await
        .expect("Database error");

    sqlx::query("DELETE FROM user_groups WHERE id = $1;")
        .bind(&groupid)
        .execute(&pool)
        .await
        .expect("Database error");

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated groups list
    let groups: Vec<UserGroupWithCount> = sqlx::query(
        "SELECT g.id, g.name, g.owner, COUNT(gm.group_id) AS member_count FROM user_groups g LEFT JOIN user_group_members gm ON g.id = gm.group_id WHERE g.owner = $1 GROUP BY g.id, g.name, g.owner ORDER BY g.created DESC;"
    )
    .bind(&user_info.login)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        UserGroupWithCount {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            member_count: row.get("member_count"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-group-members.html", escape = "none")]
struct HXGroupMembersTemplate {
    group: UserGroup,
    members: Vec<GroupMember>,
    is_owner: bool,
}

async fn hx_group_members(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let group_row: Option<sqlx::postgres::PgRow> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE id = $1;")
        .bind(&groupid)
        .fetch_optional(&pool)
        .await
        .expect("Database error");

    let group = match group_row {
        Some(row) => {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        }
        None => return Html("".as_bytes().to_vec()),
    };

    let is_owner = group.owner == user_info.login;

    let members: Vec<GroupMember> = sqlx::query("SELECT user_login FROM user_group_members WHERE group_id = $1 ORDER BY user_login;")
        .bind(&groupid)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            GroupMember {
                user_login: row.get("user_login"),
            }
        })
        .fetch_all(&pool)
        .await
        .expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_group_member(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
    Form(form): Form<AddMemberForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify group ownership
    let group_row: Option<sqlx::postgres::PgRow> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE id = $1;")
        .bind(&groupid)
        .fetch_optional(&pool)
        .await
        .expect("Database error");

    let group = match group_row {
        Some(row) => {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        }
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Check that the user exists
    let user_exists: Option<sqlx::postgres::PgRow> = sqlx::query("SELECT login FROM users WHERE login = $1;")
        .bind(&form.user_login)
        .fetch_optional(&pool)
        .await
        .expect("Database error");

    if user_exists.is_some() {
        // Insert member (ignore if already exists)
        let _ = sqlx::query("INSERT INTO user_group_members (group_id, user_login) VALUES ($1, $2) ON CONFLICT DO NOTHING;")
            .bind(&groupid)
            .bind(&form.user_login)
            .execute(&pool)
            .await;

        // Invalidate Redis group membership cache
        let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;
    }

    // Return updated members list
    let members: Vec<GroupMember> = sqlx::query("SELECT user_login FROM user_group_members WHERE group_id = $1 ORDER BY user_login;")
        .bind(&groupid)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            GroupMember {
                user_login: row.get("user_login"),
            }
        })
        .fetch_all(&pool)
        .await
        .expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_group_member(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((groupid, login)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify group ownership
    let group_row: Option<sqlx::postgres::PgRow> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE id = $1;")
        .bind(&groupid)
        .fetch_optional(&pool)
        .await
        .expect("Database error");

    let group = match group_row {
        Some(row) => {
            use sqlx::Row;
            UserGroup {
                id: row.get("id"),
                name: row.get("name"),
                owner: row.get("owner"),
            }
        }
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    sqlx::query("DELETE FROM user_group_members WHERE group_id = $1 AND user_login = $2;")
        .bind(&groupid)
        .bind(&login)
        .execute(&pool)
        .await
        .expect("Database error");

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated members list
    let members: Vec<GroupMember> = sqlx::query("SELECT user_login FROM user_group_members WHERE group_id = $1 ORDER BY user_login;")
        .bind(&groupid)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            GroupMember {
                user_login: row.get("user_login"),
            }
        })
        .fetch_all(&pool)
        .await
        .expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_user_groups_json(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> Json<Vec<UserGroup>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(vec![]);
    }
    let user_info = user_info.unwrap();

    let groups: Vec<UserGroup> = sqlx::query("SELECT id, name, owner FROM user_groups WHERE owner = $1 ORDER BY created DESC;")
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

    Json(groups)
}
