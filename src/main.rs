#![forbid(unsafe_code)]
#![allow(non_snake_case)]

use argon2::Argon2;
use argon2::PasswordHasher;
use argon2::PasswordVerifier;
use axum::http::StatusCode;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use ahash::AHashMap;
use argon2::password_hash::PasswordHash;
use askama::Template;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Form, Multipart, Path},
    http::header::HeaderMap,
    http::header::{ACCEPT_LANGUAGE, COOKIE, HOST, USER_AGENT},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    routing::post,
    Extension, Json, Router,
};
use chrono::{DateTime, Datelike, Local, Timelike};
use memory_serve::{load_assets, MemoryServe};
use meilisearch_sdk::client::Client as MeilisearchClient;
use redis::AsyncCommands;
use serde::Deserialize;
use serde::Serialize;
mod db;
use db::ScyllaDb;
use axum_server::tls_rustls::RustlsConfig;
use std::io::BufRead;
use std::sync::Arc;
use tokio::{fs, io, io::AsyncWriteExt};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

// HTTP/3 / QUIC
use std::net::SocketAddr;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use quinn::{Endpoint, ServerConfig as QuinnServerConfig};
use axum::http::{HeaderValue, Request};
use axum::http::header::HeaderName;

type RedisConn = redis::aio::ConnectionManager;

#[derive(Deserialize, Clone)]
struct MeilisearchEmbedderConfig {
    name: String,
}

#[derive(Deserialize, Clone)]
struct Config {
    scylla_nodes: Vec<String>,
    scylla_keyspace: Option<String>,
    redis_url: String,
    instancename: String,
    welcome: String,
    description: String,
    locale: String,
    custom_session_domain: Option<String>,
    meilisearch_url: String,
    meilisearch_key: Option<String>,
    /// Embedder configuration (name used for similar-document recommendations)
    meilisearch_embedder: Option<MeilisearchEmbedderConfig>,
    site_url: String,
    source_server_url: String,

    /// WebAuthn Relying Party ID (e.g. "example.com"). Required to enable WebAuthn/passkey login.
    webauthn_rp_id: Option<String>,

    /// WebAuthn Relying Party origin (e.g. "https://example.com"). Required to enable WebAuthn/passkey login.
    webauthn_rp_origin: Option<String>,

    /// Path to TLS certificate PEM file. When set (and valid), the server starts in HTTPS mode.
    tls_cert: Option<String>,

    /// Path to TLS private key PEM file. Required when tls_cert is set.
    tls_key: Option<String>,

    /// When HTTPS is active, also advertise HTTP/2 via ALPN (default: true).
    enable_http2: Option<bool>,

    /// Enable Brotli compression of HTTP responses (default: false).
    enable_brotli: Option<bool>,

    /// Enable Zstandard (zstd) compression of HTTP responses (default: false).
    enable_zstd: Option<bool>,

    /// Send Strict-Transport-Security header (max-age=31536000; includeSubDomains; preload).
    /// Only active when HTTPS is enabled (default: false).
    enable_hsts: Option<bool>,

    /// Enable HTTP/3 over QUIC (default: false).
    enable_http3: Option<bool>,
}

fn request_authority<B>(req: &Request<B>) -> Option<String> {
    req.headers()
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_string()))
}

fn request_hostname<B>(req: &Request<B>) -> String {
    request_authority(req)
        .map(|h| h.split(':').next().unwrap_or(&h).to_string())
        .unwrap_or_else(|| "localhost".to_string())
}

fn load_certs_from_pem(cert_pem: &[u8]) -> Vec<CertificateDer<'static>> {
    let mut reader = std::io::BufReader::new(cert_pem);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to parse TLS certificate PEM")
}

fn load_private_key_from_pem(key_pem: &[u8]) -> PrivateKeyDer<'static> {
    let mut reader = std::io::BufReader::new(key_pem);
    rustls_pemfile::private_key(&mut reader)
        .expect("Failed to parse private key PEM")
        .expect("No supported private key found in PEM")
}

fn build_quinn_server_config(cert_pem: &[u8], key_pem: &[u8]) -> QuinnServerConfig {
    let certs = load_certs_from_pem(cert_pem);
    let key = load_private_key_from_pem(key_pem);

    let mut tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("Failed to build rustls server config for QUIC");

    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    let quic_crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
        .expect("Failed to convert rustls config to QUIC config");

    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0u8.into());

    server_config
}

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let config: Config =
        serde_json::from_str(&fs::read_to_string("config.json").await.unwrap()).unwrap();

    // Extract TLS/compression settings before config is moved into the Extension layer.
    let tls_cert = config.tls_cert.clone();
    let tls_key = config.tls_key.clone();
    let enable_http2 = config.enable_http2.unwrap_or(true);
    let enable_http3 = config.enable_http3.unwrap_or(false);
    let enable_brotli = config.enable_brotli.unwrap_or(false);
    let enable_zstd = config.enable_zstd.unwrap_or(false);
    let enable_hsts = config.enable_hsts.unwrap_or(false);

    // HSTS is only meaningful over HTTPS; pre-compute the flag used in the middleware.
    let hsts_active = tls_cert.is_some() && enable_hsts;

    let keyspace = config.scylla_keyspace.clone().unwrap_or_else(|| "videoplatform".to_string());
    let db = ScyllaDb::connect(&config.scylla_nodes, &keyspace)
        .await
        .expect("Failed to connect to ScyllaDB");
    println!("ScyllaDB connected: nodes={:?}, keyspace={}", &config.scylla_nodes, keyspace);

    let meilisearch_client = MeilisearchClient::new(
        &config.meilisearch_url,
        config.meilisearch_key.as_deref(),
    )
    .unwrap();

    // Verify Meilisearch connectivity at startup
    match meilisearch_client.health().await {
        Ok(health) => {
            println!(
                "Meilisearch connected: url={}, status={}",
                &config.meilisearch_url, health.status
            );
        }
        Err(e) => {
            eprintln!(
                "WARNING: Meilisearch health check failed (url={}): {:?}",
                &config.meilisearch_url, e
            );
            eprintln!("Search functionality will be unavailable until Meilisearch is reachable.");
        }
    }

    // Verify the Meilisearch indexes exist
    for index_name in &["media", "lists", "users"] {
        match meilisearch_client.get_index(index_name).await {
            Ok(index) => {
                println!("Meilisearch '{}' index found: uid={}", index_name, index.uid);
            }
            Err(e) => {
                eprintln!(
                    "WARNING: Meilisearch '{}' index not accessible: {:?}",
                    index_name, e
                );
                eprintln!(
                    "Ensure the '{}' index is created and populated by the indexer.",
                    index_name
                );
            }
        }
    }

    let redis_client = redis::Client::open(config.redis_url.as_str()).unwrap();
    let redis_conn = redis_client.get_connection_manager().await.unwrap();
    println!("Redis connected: url={}", &config.redis_url);

    // Build the localization service (parses embedded FTL files at startup)
    let localization = LocalizationService::new();
    println!(
        "Localization: {} language(s) loaded: {}",
        localization.available_langs.len(),
        localization.available_langs.iter().map(|l| l.code.as_str()).collect::<Vec<_>>().join(", ")
    );

    // Build WebAuthn instance (optional – only when rp_id and rp_origin are configured)
    let webauthn_instance: Option<webauthn_rs::Webauthn> =
        match (&config.webauthn_rp_id, &config.webauthn_rp_origin) {
            (Some(rp_id), Some(rp_origin_str)) => {
                let rp_origin = url::Url::parse(rp_origin_str)
                    .expect("Invalid webauthn_rp_origin in config.json");
                let webauthn = webauthn_rs::WebauthnBuilder::new(rp_id, &rp_origin)
                    .expect("Invalid WebAuthn configuration")
                    .rp_name(&config.instancename)
                    .build()
                    .expect("Failed to build WebAuthn");
                println!("WebAuthn enabled: rp_id={}, origin={}", rp_id, rp_origin_str);
                Some(webauthn)
            }
            _ => {
                println!("WebAuthn disabled (webauthn_rp_id / webauthn_rp_origin not set in config.json)");
                None
            }
        };

    let webauthn_ext = std::sync::Arc::new(std::sync::RwLock::new(webauthn_instance));

    let memory_router = MemoryServe::new(load_assets!("assets/processed")).into_router();

    let has_tls = tls_cert.is_some();
    let app = Router::new()
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/", get(home))
        .route("/login", get(login))
        .route("/trending", get(trending))
        .route("/hx/trending", get(hx_trending))
        .route("/hx/trending/{page}", get(hx_trending_page))
        .route("/subscriptions", get(subscriptions))
        .route("/hx/subscriptions", get(hx_subscriptions))
        .route("/hx/subscriptions/{page}", get(hx_subscriptions_page))
        .route("/history", get(history))
        .route("/hx/history", get(hx_history))
        .route("/hx/history/{page}", get(hx_history_page))
        .route("/m/{mediumid}", get(medium))
        .route("/m/{mediumid}/previews.vtt", get(medium_previews_prepare))
        .route(
            "/m/{mediumid}/description.json",
            get(medium_description_prepare),
        )
        .route("/m/{mediumid}/video.mp4", get(compose_mp4))
        .route("/m/{mediumid}/video-sm.mp4", get(compose_mp4_sm))
        .route("/hx/comments/{mediumid}", get(hx_comments))
        .route("/hx/comments/{mediumid}/add", post(hx_add_comment))
        .route("/hx/comment/{commentid}/delta.json", get(comment_delta))
        .route("/hx/reccomended/{mediumid}", get(hx_recommended))
        .route("/hx/new_view/{mediumid}", get(hx_new_view))
        .route("/hx/likedislikebutton/{mediumid}", get(hx_likedislikebutton))
        .route("/hx/like/{mediumid}", get(hx_like))
        .route("/hx/dislike/{mediumid}", get(hx_dislike))
        .route("/hx/subscribe/{userid}", get(hx_subscribe))
        .route("/hx/unsubscribe/{userid}", get(hx_unsubscribe))
        .route("/hx/subscribebutton/{userid}", get(hx_subscribebutton))
        .route("/hx/login", post(hx_login))
        .route("/hx/login/2fa/totp", post(hx_login_2fa_totp))
        .route("/hx/logout", get(hx_logout))
        .route("/hx/usernav", get(hx_usernav))
        .route("/hx/sidebar/{active_item}", get(hx_sidebar))
        .route("/hx/searchsuggestions", post(hx_search_suggestions))
        .route("/search", get(search))
        .route("/hx/search/{pageid}", post(hx_search))
        .route("/u/{userid}", get(channel))
        .route("/hx/usermedia/{userid}", get(hx_usermedia))
        .route("/hx/usermedia/{userid}/{page}", get(hx_usermedia_page))
        .route("/studio", get(studio))
        .route("/hx/studio", get(hx_studio))
        .route("/hx/studio/{page}", get(hx_studio_page))
        .route("/studio/edit/{mediumid}", get(studio_edit))
        .route("/studio/edit/{mediumid}", post(studio_edit_save))
        .route("/studio/edit/{mediumid}/chapters.json", get(studio_chapters_get))
        .route("/studio/edit/{mediumid}/chapters", post(studio_chapters_save))
        .route("/studio/edit/{mediumid}/subtitles.json", get(studio_subtitles_get))
        .route("/studio/edit/{mediumid}/subtitles/add", post(studio_subtitles_add))
        .route(
            "/studio/edit/{mediumid}/subtitles/translate/status",
            get(studio_subtitles_translate_status),
        )
        .route(
            "/studio/edit/{mediumid}/subtitles/translate",
            post(studio_subtitles_translate),
        )
        .route(
            "/studio/edit/{mediumid}/subtitles/delete",
            post(studio_subtitles_delete),
        )
        .route(
            "/studio/edit/{mediumid}/subtitles/font.json",
            get(studio_subtitle_font_get),
        )
        .route(
            "/studio/edit/{mediumid}/subtitles/font",
            post(studio_subtitle_font_upload),
        )
        .route(
            "/studio/edit/{mediumid}/subtitles/font/delete",
            post(studio_subtitle_font_delete),
        )
        .route("/studio/edit/{mediumid}/thumbnail.json", get(studio_thumbnail_get))
        .route("/studio/edit/{mediumid}/thumbnail", post(studio_thumbnail_upload))
        .route(
            "/studio/edit/{mediumid}/thumbnail/delete",
            post(studio_thumbnail_delete),
        )
        .route("/hx/studio/delete/{mediumid}", get(hx_delete_video))
        .route(
            "/hx/studio/edit/{mediumid}/description",
            get(hx_studio_edit_description),
        )
        .route(
            "/hx/studio/edit/{mediumid}/chapters",
            get(hx_studio_edit_chapters_tab),
        )
        .route(
            "/hx/studio/edit/{mediumid}/subtitles",
            get(hx_studio_edit_subtitles_tab),
        )
        .route(
            "/hx/studio/edit/{mediumid}/thumbnail",
            get(hx_studio_edit_thumbnail_tab),
        )
        .route(
            "/hx/studio/edit/{mediumid}/permissions",
            get(hx_studio_edit_permissions_tab),
        )
        .route(
            "/studio/edit/{mediumid}/permissions",
            post(studio_edit_permissions_save),
        )
        .route(
            "/hx/studio/edit/{mediumid}/danger",
            get(hx_studio_edit_danger_tab),
        )
        .route("/studio/lists", get(studio_lists))
        .route("/hx/studio/lists", get(hx_studio_lists))
        .route("/hx/studio/lists/{page}", get(hx_studio_lists_page))
        .route("/studio/concepts", get(concepts))
        .route("/hx/studio/concepts", get(hx_concepts))
        .route("/studio/concept/{conceptid}", get(concept))
        .route("/studio/concept/{conceptid}/publish", post(publish))
        .route("/upload", get(upload))
        .route("/hx/upload", post(hx_upload))
        .route("/hx/studio/upload", get(hx_studio_upload))
        .route("/l/{listid}", get(list_page))
        .route("/l/{listid}/{mediumid}", get(medium_in_list))
        .route("/hx/listitems/{listid}", get(hx_list_items))
        .route("/hx/listitems/{listid}/{page}", get(hx_list_items_page))
        .route("/hx/listsidebar/{listid}/{mediumid}", get(hx_list_sidebar))
        .route("/hx/listmodal/{mediumid}", get(hx_list_modal))
        .route("/hx/createlist/{mediumid}", post(hx_create_list))
        .route("/hx/addtolist/{listid}/{mediumid}", get(hx_add_to_list))
        .route("/hx/removefromlist/{listid}/{mediumid}", get(hx_remove_from_list))
        .route("/hx/deletelist/{listid}", get(hx_delete_list))
        .route("/hx/userlists/{userid}", get(hx_user_lists))
        .route("/hx/userlists/{userid}/{page}", get(hx_user_lists_page))
        .route("/user-style.css", get(user_style_css))
        .route("/settings", get(settings))
        .route("/settings/password", get(settings_password))
        .route("/settings/profile-picture", get(settings_profile_picture))
        .route("/settings/channel-picture", get(settings_channel_picture))
        .route("/settings/diagnostics", get(settings_diagnostics))
        .route("/settings/theme", get(settings_theme))
        .route("/settings/language", get(settings_language))
        .route("/settings/2fa", get(settings_2fa))
        .route("/hx/settings/channel-name", get(hx_settings_channel_name))
        .route("/hx/settings/channel-name", post(hx_settings_channel_name_save))
        .route("/hx/settings/password", get(hx_settings_password))
        .route("/hx/settings/password", post(hx_settings_password_save))
        .route("/hx/settings/profile-picture", get(hx_settings_profile_picture))
        .route("/hx/settings/profile-picture", post(hx_settings_profile_picture_save))
        .route("/hx/settings/channel-picture", get(hx_settings_channel_picture))
        .route("/hx/settings/channel-picture", post(hx_settings_channel_picture_save))
        .route("/hx/settings/diagnostics", get(hx_settings_diagnostics))
        .route("/hx/settings/theme", get(hx_settings_theme))
        .route("/hx/settings/theme", post(hx_settings_theme_save))
        .route("/hx/settings/language", get(hx_settings_language))
        .route("/hx/settings/language", post(hx_settings_language_save))
        .route("/hx/settings/2fa", get(hx_settings_2fa))
        .route("/hx/settings/2fa/totp/setup", post(hx_settings_2fa_totp_setup))
        .route(
            "/hx/settings/2fa/totp/verify-setup",
            post(hx_settings_2fa_totp_verify_setup),
        )
        .route("/hx/settings/2fa/totp/disable", post(hx_settings_2fa_totp_disable))
        .route(
            "/hx/settings/2fa/webauthn/delete/{credid}",
            post(hx_settings_2fa_webauthn_delete),
        )
        .route("/hx/webauthn/register/start", post(hx_webauthn_register_start))
        .route("/hx/webauthn/register/finish", post(hx_webauthn_register_finish))
        .route("/hx/webauthn/auth/start", post(hx_webauthn_auth_start))
        .route("/hx/webauthn/auth/finish", post(hx_webauthn_auth_finish))
        .route("/studio/groups", get(studio_groups))
        .route("/hx/studio/groups", get(hx_studio_groups))
        .route("/hx/creategroup", post(hx_create_group))
        .route("/hx/deletegroup/{groupid}", get(hx_delete_group))
        .route("/hx/group/{groupid}/members", get(hx_group_members))
        .route("/hx/group/{groupid}/addmember", post(hx_add_group_member))
        .route("/hx/group/{groupid}/removemember/{login}", get(hx_remove_group_member))
        .route("/hx/usergroups.json", get(hx_user_groups_json))
        .nest("/source", static_router("source"))
        .layer(Extension(db))
        .layer(Extension(config))
        .layer(Extension(redis_conn))
        .layer(Extension(localization))
        .layer(Extension(Arc::new(meilisearch_client)))
        .layer(Extension(webauthn_ext))
        .layer(DefaultBodyLimit::disable())
        .merge(memory_router)
        .layer(
            CompressionLayer::new()
                .br(enable_brotli)
                .zstd(enable_zstd),
        )
        .layer(axum::middleware::from_fn(
            move |mut req: axum::http::Request<Body>, next: Next| async move {
                if enable_brotli && enable_zstd {
                    if let Some(enc_val) = req.headers().get("accept-encoding") {
                        let enc = enc_val.to_str().unwrap_or("").to_string();
                        if enc.to_ascii_lowercase().contains("zstd")
                            && enc.to_ascii_lowercase().contains("br")
                        {
                            let filtered: String = enc
                                .split(',')
                                .map(str::trim)
                                .filter(|part| {
                                    let token = part.split(';').next().unwrap_or(part).trim();
                                    !token.eq_ignore_ascii_case("br")
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            if let Ok(val) = axum::http::header::HeaderValue::from_str(&filtered)
                            {
                                req.headers_mut().insert(
                                    axum::http::header::HeaderName::from_static("accept-encoding"),
                                    val,
                                );
                            }
                        }
                    }
                }
                next.run(req).await
            },
        ))
        .layer(axum::middleware::from_fn(
            move |req: axum::http::Request<Body>, next: Next| async move {
                let mut response = next.run(req).await;

                if hsts_active {
                    response.headers_mut().insert(
                        axum::http::header::HeaderName::from_static(
                            "strict-transport-security",
                        ),
                        axum::http::header::HeaderValue::from_static(
                            "max-age=31536000; includeSubDomains; preload",
                        ),
                    );
                }

                if has_tls && enable_http3 {
                    response.headers_mut().insert(
                        axum::http::header::HeaderName::from_static("alt-svc"),
                        axum::http::header::HeaderValue::from_static(r#"h3=":443"; ma=86400"#),
                    );
                }

                response
            },
        ));

    if let Some(cert_path) = tls_cert {
        let key_path = tls_key
            .expect("config: tls_key must be set when tls_cert is provided");

        let cert_bytes = tokio::fs::read(&cert_path)
            .await
            .expect("Failed to read TLS certificate file");
        let key_bytes = tokio::fs::read(&key_path)
            .await
            .expect("Failed to read TLS private key file");

        // Load TLS configuration from PEM data.
        // RustlsConfig::from_pem sets ALPN to ["h2", "http/1.1"] by default [1].
        let rustls_config = RustlsConfig::from_pem(cert_bytes.clone(), key_bytes.clone())
            .await
            .expect("Failed to configure TLS: ensure cert and key are valid PEM files");

        // When HTTP/2 is disabled, override ALPN to advertise http/1.1 only [1].
        if !enable_http2 {
            let mut server_cfg = (*rustls_config.get_inner()).clone();
            server_cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
            rustls_config.reload_from_config(Arc::new(server_cfg));
        }

        let https_addr: SocketAddr = "0.0.0.0:8443".parse().unwrap();
        let http_addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();

        // Plain-HTTP server on :8080 — redirects every request to HTTPS on :8443 [1].
        let redirect_app = Router::new().fallback(
            |req: axum::http::Request<Body>| async move {
                let hostname = request_hostname(&req);
                let path_and_query = req
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/");
                let url = format!("https://{}:8443{}", hostname, path_and_query);
                Redirect::permanent(&url)
            },
        );

        let redirect_listener = tokio::net::TcpListener::bind(http_addr)
            .await
            .unwrap();

        println!(
            "HTTP  redirect on: http://{}  →  https://<host>:8443",
            http_addr
        );
        println!(
            "HTTPS listening on: https://{} (HTTP/2: {}, HTTP/3: {}, brotli: {}, zstd: {}, HSTS: {})",
            https_addr, enable_http2, enable_http3, enable_brotli, enable_zstd, hsts_active
        );

        // Spawn the plain-HTTP redirect server; run the HTTPS server in the foreground [1].
        tokio::spawn(async move {
            axum::serve(redirect_listener, redirect_app)
                .await
                .unwrap();
        });

        // Optional HTTP/3 server on UDP/8443.
        if enable_http3 {
            let app_h3 = app.clone();

            tokio::spawn(async move {
                let quinn_config = build_quinn_server_config(&cert_bytes, &key_bytes);
                let endpoint = Endpoint::server(quinn_config, https_addr)
                    .expect("Failed to bind QUIC endpoint for HTTP/3");

                println!("HTTP/3 listening on: https://{} over QUIC/UDP", https_addr);

                let acceptor = h3_util::quinn::H3QuinnAcceptor::new(endpoint);
                axum_h3::H3Router::new(app_h3)
                    .serve(acceptor)
                    .await
                    .expect("HTTP/3 server failed");
            });
        }

        axum_server::bind_rustls(https_addr, rustls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        println!(
            "Listening on: http://{} (brotli: {}, zstd: {})",
            listener.local_addr().unwrap(),
            enable_brotli,
            enable_zstd
        );
        axum::serve(listener, app).await.unwrap();
    }
}

include!("helper_functions.rs");
include!("sidebar.rs");
include!("medium.rs");
include!("comments.rs");
include!("reccomendations.rs");
include!("likes_dislikes.rs");
include!("subscriptions.rs");
include!("views.rs");
include!("login_handler.rs");
include!("usernav.rs");
include!("trending.rs");
include!("home.rs");
include!("search.rs");
include!("channel.rs");
include!("studio.rs");
include!("chapters.rs");
include!("subtitles.rs");
include!("thumbnail.rs");
include!("upload.rs");
include!("concept.rs");
include!("serve.rs");
include!("mp4_compose.rs");
include!("lists.rs");
include!("groups.rs");
include!("localization.rs");
include!("settings.rs");
include!("two_factor.rs");
include!("sitemap.rs");
include!("history.rs");
