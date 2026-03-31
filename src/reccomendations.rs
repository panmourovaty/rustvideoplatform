async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    Extension(meili): Extension<Arc<MeilisearchClient>>,
    Extension(localization): Extension<Arc<LocalizationService>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let mediumid = mediumid.to_ascii_lowercase();
    let common_headers = extract_common_headers(&headers);
    let user = get_user_login(headers, &db, redis).await;
    let visibility_filter = build_visibility_filter(&db, &user).await;

    let embedder = config
        .meilisearch_embedder
        .as_ref()
        .map(|embedder| embedder.name.as_str())
        .unwrap_or("default")
        .to_owned();

    let index = meili.index("media");
    let mut query =
        meilisearch_sdk::similar::SimilarQuery::new(&index, &mediumid, &embedder);
    query.with_limit(20);
    query.with_filter(&visibility_filter);

    let media: Vec<Medium> = query
        .execute::<MeiliMedia>()
        .await
        .map(|results| {
            results
                .hits
                .into_iter()
                .map(|hit| hit.result.into())
                .collect()
        })
        .unwrap_or_default();

    let locale = resolve_locale_noauth(
        common_headers.accept_language.as_deref(), &config.locale, &localization,
    );
    let template = HXMediumListTemplate {
        current_medium_id: mediumid,
        list_id: String::new(),
        media,
        config,
        locale,
    };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}
