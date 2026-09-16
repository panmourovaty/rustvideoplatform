# Rust Video Platform

A self-hosted multimedia platform built with Axum, ScyllaDB, Redis-compatible
storage, and Meilisearch.

## Requirements

- Rust 1.95 or newer
- ScyllaDB with `database_schema.cql` applied
- Redis or Dragonfly
- Meilisearch
- FFmpeg and `woff2_compress`
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

Video and audio playback use the official [Video.js HTML/CSS integration](https://videojs.org/docs/framework/html/how-to/installation),
pinned to `10.0.0-rc.2`. The CDN scripts and stylesheet are declared in
`templates/pages/component-dependencies-player.html`. CMAF videos use `hlsjs-video`,
legacy MPEG-DASH videos use `dash-video`, and audio uses the native `audio` element.
Video.js v10 is currently a release candidate; its DASH adapter is still beta.

`component-player-controls.html` contains an ejected minimal video skin, following
the documented [skin customization path](https://videojs.org/docs/framework/html/how-to/customize-skins).
Its controls render directly in the page and use the versioned `video-minimal.css`;
application overrides in `assets/static/style.css` preserve the full-width control
bar and persistent audio cover image. Update the copied markup and all Video.js
CDN URLs together when upgrading. Upstream attribution is in `LICENSES/videojs.txt`.

The player retains captions, chapters, thumbnail previews, playback settings,
keyboard/touch controls, fullscreen, picture-in-picture, autoplay, and `?t=`
timestamp seeking. ASS/SSA subtitles use JASSUB with the uploaded font when
available; its WebAssembly renderer requires the CSP's `wasm-unsafe-eval` permission.
Autoplay and picture-in-picture remain subject to browser support and policy.

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
