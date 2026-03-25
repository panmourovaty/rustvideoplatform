async fn robots_txt(Extension(config): Extension<Config>) -> Response {
    let body = format!(
        "User-agent: *\nAllow: /\n\nSitemap: {}/sitemap.xml\n",
        config.site_url
    );
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn sitemap_xml(Extension(mut redis): Extension<RedisConn>) -> Response {
    let cached: Option<String> = redis.get("cache:sitemap").await.unwrap_or(None);

    match cached {
        Some(xml) => (
            axum::http::StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/xml; charset=utf-8",
            )],
            xml,
        )
            .into_response(),
        None => axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
