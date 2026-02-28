#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
    r#type: String,
}

async fn concepts(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
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
        active_tab: "concepts".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html", escape = "none")]
struct HXConceptsTemplate {
    concepts: Vec<MediumConcept>,
}
async fn hx_concepts(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let userinfo = get_user_login(headers, &pool, redis.clone()).await.unwrap();

    let concepts = sqlx::query_as!(
        MediumConcept,
        "SELECT id,name,processed,type FROM media_concepts WHERE owner = $1;",
        userinfo.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
    let template = HXConceptsTemplate { concepts };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/concept.html", escape = "none")]
struct ConceptTemplate {
    sidebar: String,
    config: Config,
    concept: MediumConcept,
    common_headers: CommonHeaders,
    owner_groups: Vec<UserGroup>,
}
async fn concept(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    Extension(redis): Extension<RedisConn>,
    Path(conceptid): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let concept = sqlx::query_as!(MediumConcept,
        "SELECT id,name,type,processed FROM media_concepts WHERE owner = $1 AND id = $2 AND processed = true;", user_info.login, conceptid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    // Fetch user's groups for the dropdown
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

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ConceptTemplate {
        sidebar,
        config,
        concept,
        common_headers,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PublishForm {
    medium_id: String,
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}
async fn publish(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(conceptid): Path<String>,
    Form(form): Form<PublishForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let concept = sqlx::query_as!(MediumConcept,
        "SELECT id,name,type,processed FROM media_concepts WHERE owner = $1 AND id = $2 AND processed = true;", user_info.login, conceptid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let concept_move = move_dir(
        format!("upload/{}_processing", conceptid).as_str(),
        format!("source/{}", form.medium_id.to_ascii_lowercase()).as_str(),
    )
    .await;
    if concept_move.is_ok() {
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
            serde_json::from_str(&form.medium_description).unwrap();
        let _ = sqlx::query(
            "INSERT INTO media (id,name,description,owner,visibility,restricted_to_group,type) VALUES ($1,$2,$3,$4,$5,$6,$7);"
        )
        .bind(form.medium_id.to_ascii_lowercase())
        .bind(&form.medium_name)
        .bind(&description)
        .bind(&user_info.login)
        .bind(&visibility)
        .bind(&restricted_to_group)
        .bind(&concept.r#type)
        .execute(&pool)
        .await;
        let _ = sqlx::query!("DELETE FROM media_concepts WHERE id=$1;", concept.id)
            .execute(&pool)
            .await;
        return Html(format!(
            "<script>window.location.replace(\"/m/{}\");</script>",
            form.medium_id
        ));
    } else {
        return Html(format!(
            "<h1>ERROR MOVING PROCESSED CONCEPT TO MEDIA!</h1><br>{:?}",
            concept_move
        ));
    }
}
