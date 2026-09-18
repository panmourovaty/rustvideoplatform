# Rust Video Platform

A self-hosted multimedia platform built with Axum, ScyllaDB, Redis-compatible
storage, and Meilisearch.

## Requirements

- Rust 1.95 or newer
- ScyllaDB with `database_schema.cql` applied
- Redis or Dragonfly
- Meilisearch
- FFmpeg, `woff2_compress`, and `woff2_decompress`
- Node.js/npm for optimized CSS and JavaScript builds

## Local build

1. Apply `database_schema.cql` to a ScyllaDB keyspace.
2. Copy `example-deployment/config.json` to `config.json` and set the public URL,
   database nodes, Redis URL, and Meilisearch settings.
3. Set `MEILISEARCH_KEY` in the environment instead of storing the key in
   `config.json`.
4. Run `cargo build --release`.
5. Create writable `source/` and `upload/` directories and run
   `target/release/rustvideoplatform` from the project directory.

The application serves media from its public `/source` route by default. Set
`source_server_url` to the origin of a separate static server or CDN to generate
media URLs against that server instead.

## Playback UI

The medium page uses the official Video.js HTML custom elements and CSS,
with every Video.js CDN asset following the `@videojs/cdn@next` release channel. The player provider
loads before the UI bundle so the controls attach to the existing player.
HLS/CMAF uses `hlsjs-video`, older MPEG-DASH media uses `dash-video`, and audio
uses native Ogg playback. Upstream v10 is a release candidate; its DASH adapter
is still labeled beta.

`templates/pages/component-player-controls.html` owns the editable minimal
skin markup, including settings, quality, captions, chapters, thumbnail previews,
keyboard shortcuts, PiP, and fullscreen. Layout overrides live in
`assets/static/style.css`; ASS/JASSUB subtitles and timestamp resume links are
handled in `assets/static/script.js`. Native video elements and their tracks are
present in the page before the streaming modules load.

ASS subtitles use JASSUB 2 / abslink 1 bundles from esm.sh, with major-only
version ranges shared by the renderer, worker, and WebAssembly URLs. The CSP permits
WebAssembly and that worker origin. Uploaded WOFF2 fonts remain available for
browser captions; `/m/{id}/subtitle-font.ttf` converts them with `woff2_decompress`
for libass, including existing uploads. Font conversion runs in a temporary file
with a ten-second timeout. To exercise it, run
`cargo test uploaded_woff2_is_decoded_for_libass -- --include-ignored` with the
runtime tool on PATH.

When the Video.js release channel updates, compare the copied controls against
the new release's minimal skin. The copied markup is Apache-2.0 licensed
(see `VIDEOJS-LICENSE`). See the official
[HTML installation](https://videojs.org/docs/framework/html/how-to/installation),
[CDN](https://videojs.org/docs/framework/html/concepts/cdn), and
[skin customization](https://videojs.org/docs/framework/html/how-to/customize-skins)
guides. No sandbox deployment or private shadow-DOM patching is used.

## Container deployment

The runtime image uses UID/GID `10001`. Bind-mounted directories must be writable
by that account:

```sh
mkdir -p source upload scylladb dragonfly
sudo chown -R 10001:10001 source upload
export MEILI_MASTER_KEY='replace-with-a-long-random-secret'
export RUSTVIDEO_PROCESSOR_DIGEST='sha256:replace-with-published-digest'
export RUSTVIDEO_INDEXER_DIGEST='sha256:replace-with-published-digest'
docker compose -f example-deployment/docker-compose.yml up -d
```

The example builds the web application from this checkout. Set the processor and
indexer digests to the immutable digests for the releases you intend to deploy.
Keep ScyllaDB, Redis/Dragonfly, Meilisearch, and model servers on an internal
container network. Terminate TLS at a trusted reverse proxy or configure the
application TLS fields directly. `site_url` must match the browser-visible
origin because unsafe HTTP methods use strict same-origin validation.

## Security-related configuration

- `site_url`: absolute public `http` or `https` URL.
- `session_ttl_seconds`: optional session lifetime; defaults to 24 hours.
- `custom_session_domain`: omit unless sessions intentionally span subdomains.
- `source_server_url`: optional absolute `http` or `https` base URL for the
  server or CDN exposing the public `/source` directory. Do not include
  `/source` itself. The external server must allow cross-origin media and fetch
  requests from `site_url`.
- `enable_hsts`: enable only after HTTPS is working for every relevant subdomain.
