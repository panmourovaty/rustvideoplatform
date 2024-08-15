#[derive(Serialize, Deserialize)]
struct HXSearch {
    search: String,
}
#[derive(Template)]
#[template(path = "pages/hx-searchsuggestion.html", escape = "none")]
struct HXSearchSuggestions {
    suggestions: Vec<Medium>,
}
async fn hx_search_suggestions(
    Extension(pool): Extension<PgPool>,
    Form(form): Form<HXSearch>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let search_term = format!("%{}%", form.search);
    let suggestions = sqlx::query_as!(
        Medium,
        "SELECT id,name,owner,views,type FROM media WHERE name ILIKE $1 LIMIT 5;",
        search_term
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| vec![]);

    if suggestions.is_empty() {
        return Html("<li><b>Nothing found</b></li>".to_owned());
    }

    let template = HXSearchSuggestions { suggestions };
    Html(template.render().unwrap())
}

#[derive(Template)]
#[template(path = "pages/hx-search.html", escape = "none")]
struct HXSearchTemplate {
    search_results: Vec<Medium>,
    next_page: i64,
    search_term: String,
}
async fn hx_search(
    Extension(pool): Extension<PgPool>,
    Path(pageid): Path<i64>,
    Form(form): Form<HXSearch>,
) -> axum::response::Html<String> {
    if form.search.trim().is_empty() {
        return Html("".to_owned());
    }

    let offset = pageid * 10;
    let next_page = pageid + 1;

    let search_querry = format!("%{}%", form.search);
    let search_results = sqlx::query_as!(
        Medium,
        "SELECT id,name,owner,views,type FROM media WHERE name ILIKE $1 LIMIT 10 OFFSET $2;",
        search_querry,
        offset
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| vec![]);

    if search_results.is_empty() {
        return Html("<li><b>Nothing found</b></li>".to_owned());
    }

    let template = HXSearchTemplate {
        search_results,
        next_page,
        search_term: form.search,
    };
    Html(template.render().unwrap())
}

#[derive(Template)]
#[template(path = "pages/search.html", escape = "none")]
struct SearchTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn search(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "search".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SearchTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}