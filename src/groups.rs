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
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers);
    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let resolved_lang = locale.lang.clone();
    let sidebar = generate_sidebar(&config, "studio".to_owned(), locale.clone());
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "groups".to_owned(),
        locale,
        resolved_lang,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-groups.html", escape = "none")]
struct HXStudioGroupsTemplate {
    groups: Vec<UserGroupWithCount>,
}

#[derive(Template)]
#[template(path = "pages/hx-groups-list.html", escape = "none")]
struct HXGroupsListTemplate {
    groups: Vec<UserGroupWithCount>,
}

/// Helper: fetch groups for owner with member counts, prepending system groups.
async fn fetch_groups_with_counts(db: &ScyllaDb, owner: &str) -> Vec<UserGroupWithCount> {
    let mut groups: Vec<UserGroupWithCount> = vec![
        UserGroupWithCount {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: owner.to_owned(),
            member_count: None,
        },
        UserGroupWithCount {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: owner.to_owned(),
            member_count: None,
        },
    ];

    // Fetch groups from user_groups_by_owner: (id, name, created)
    let user_groups: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_groups_by_owner, (owner,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for (id, name, _created) in user_groups {
        // Count members for this group
        let members: Vec<(String,)> = db.session
            .execute_unpaged(&db.get_group_members, (&id,)).await
            .ok().and_then(|r| r.into_rows_result().ok())
            .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
            .unwrap_or_default();

        groups.push(UserGroupWithCount {
            id,
            name,
            owner: owner.to_owned(),
            member_count: Some(members.len() as i64),
        });
    }

    groups
}

async fn hx_studio_groups(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_group(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<CreateGroupForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let group_id = generate_medium_id();
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Insert into user_groups (id, name, owner, created)
    let _ = db.session
        .execute_unpaged(&db.insert_group, (&group_id, &form.name, &user_info.login, created))
        .await;

    // Insert into user_groups_by_owner (owner, created, id, name)
    let _ = db.session
        .execute_unpaged(&db.insert_group_by_owner, (&user_info.login, created, &group_id, &form.name))
        .await;

    // Return updated groups list
    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_group(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Prevent deletion of system groups
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify ownership via get_group_by_id: (id, name, owner)
    let group_info = db.session
        .execute_unpaged(&db.get_group_by_id, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String)>().ok().flatten());

    let (_id, _name, owner) = match group_info {
        Some(row) => row,
        None => return Html("".as_bytes().to_vec()),
    };

    if owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Clear group references from media
    let media_rows: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_media_by_group, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for (media_id, _media_owner, _upload) in &media_rows {
        // update_media_permissions: (public, visibility, restricted_to_group, id)
        let _ = db.session
            .execute_unpaged(&db.update_media_permissions, (false, "hidden", None::<&str>, media_id))
            .await;
    }

    // Clear group references from lists
    let list_rows: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_lists_by_group, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for (list_id, _list_owner, _created) in &list_rows {
        let _ = db.session
            .query_unpaged("UPDATE lists SET visibility = 'hidden', restricted_to_group = null WHERE id = ?", (list_id,))
            .await;
    }

    // Delete all members
    let members: Vec<(String,)> = db.session
        .execute_unpaged(&db.get_group_members, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    for (member_login,) in &members {
        let _ = db.session
            .execute_unpaged(&db.delete_group_member, (&groupid, member_login))
            .await;
        let _ = db.session
            .execute_unpaged(&db.delete_group_by_member, (member_login, &groupid))
            .await;
    }

    // Find created timestamp from get_groups_by_owner to delete from by_owner table
    let owner_groups: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_groups_by_owner, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    if let Some((_, _, created)) = owner_groups.iter().find(|(id, _, _)| id == &groupid) {
        // delete_group_by_owner: (owner, created, id)
        let _ = db.session
            .execute_unpaged(&db.delete_group_by_owner, (&user_info.login, *created, &groupid))
            .await;
    }

    // Delete group from main table
    let _ = db.session
        .execute_unpaged(&db.delete_group, (&groupid,))
        .await;

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated groups list
    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

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
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // System groups are automatically managed - show info instead of member list
    if is_system_group(&groupid) {
        let group = if groupid == SYSTEM_GROUP_ALL_REGISTERED {
            UserGroup { id: groupid, name: "All Registered Users".to_owned(), owner: user_info.login.clone() }
        } else {
            UserGroup { id: groupid, name: "Subscribers Only".to_owned(), owner: user_info.login.clone() }
        };
        let template = HXGroupMembersTemplate {
            group,
            members: vec![],
            is_owner: false, // prevents showing add/remove controls
        };
        return Html(minifi_html(template.render().unwrap()));
    }

    // Get group: (id, name, owner)
    let group_row = db.session
        .execute_unpaged(&db.get_group_by_id, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String)>().ok().flatten());

    let group = match group_row {
        Some((id, name, owner)) => UserGroup { id, name, owner },
        None => return Html("".as_bytes().to_vec()),
    };

    let is_owner = group.owner == user_info.login;

    // Get members: (user_login)
    let member_rows: Vec<(String,)> = db.session
        .execute_unpaged(&db.get_group_members, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut members: Vec<GroupMember> = member_rows.into_iter()
        .map(|(user_login,)| GroupMember { user_login })
        .collect();
    members.sort_by(|a, b| a.user_login.cmp(&b.user_login));

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_group_member(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
    Form(form): Form<AddMemberForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // System groups cannot have members manually added
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify group ownership: (id, name, owner)
    let group_row = db.session
        .execute_unpaged(&db.get_group_by_id, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String)>().ok().flatten());

    let group = match group_row {
        Some((id, name, owner)) => UserGroup { id, name, owner },
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Check that the user exists: (login)
    let user_exists = db.session
        .execute_unpaged(&db.check_user_exists, (&form.user_login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    if user_exists.is_some() {
        // Insert member (Cassandra INSERT is an upsert)
        let _ = db.session
            .execute_unpaged(&db.insert_group_member, (&groupid, &form.user_login))
            .await;
        let _ = db.session
            .execute_unpaged(&db.insert_group_by_member, (&form.user_login, &groupid))
            .await;

        // Invalidate Redis group membership cache
        let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;
    }

    // Return updated members list
    let member_rows: Vec<(String,)> = db.session
        .execute_unpaged(&db.get_group_members, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut members: Vec<GroupMember> = member_rows.into_iter()
        .map(|(user_login,)| GroupMember { user_login })
        .collect();
    members.sort_by(|a, b| a.user_login.cmp(&b.user_login));

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_group_member(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((groupid, login)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // System groups cannot have members manually removed
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify group ownership: (id, name, owner)
    let group_row = db.session
        .execute_unpaged(&db.get_group_by_id, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String)>().ok().flatten());

    let group = match group_row {
        Some((id, name, owner)) => UserGroup { id, name, owner },
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Delete member from both tables
    let _ = db.session
        .execute_unpaged(&db.delete_group_member, (&groupid, &login))
        .await;
    let _ = db.session
        .execute_unpaged(&db.delete_group_by_member, (&login, &groupid))
        .await;

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated members list
    let member_rows: Vec<(String,)> = db.session
        .execute_unpaged(&db.get_group_members, (&groupid,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String,)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut members: Vec<GroupMember> = member_rows.into_iter()
        .map(|(user_login,)| GroupMember { user_login })
        .collect();
    members.sort_by(|a, b| a.user_login.cmp(&b.user_login));

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_user_groups_json(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> Json<Vec<UserGroup>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(vec![]);
    }
    let user_info = user_info.unwrap();

    let mut groups = system_groups_for_owner(&user_info.login);

    // Fetch groups from user_groups_by_owner: (id, name, created)
    let user_groups: Vec<(String, String, i64)> = db.session
        .execute_unpaged(&db.get_groups_by_owner, (&user_info.login,)).await
        .ok().and_then(|r| r.into_rows_result().ok())
        .map(|rows| rows.rows::<(String, String, i64)>().unwrap().filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();

    groups.extend(user_groups.into_iter().map(|(id, name, _created)| UserGroup {
        id,
        name,
        owner: user_info.login.clone(),
    }));

    Json(groups)
}
