-- Cassandra / ScyllaDB schema for rustvideoplatform
-- Designed for query-driven access patterns with denormalization.
-- Use NetworkTopologyStrategy for multi-datacenter deployments.

-- For single-node / development deployments use SimpleStrategy with RF=1.
-- For production multi-datacenter clusters switch to:
--   {'class': 'NetworkTopologyStrategy', '<dc_name>': 3}
CREATE KEYSPACE IF NOT EXISTS videoplatform
  WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}
  AND durable_writes = true;

USE videoplatform;

-- ══════════════════════════════════════════════════════════════════════
-- USERS
-- ══════════════════════════════════════════════════════════════════════

-- Primary user lookup by login (partition key = login)
CREATE TABLE IF NOT EXISTS users (
    login               text PRIMARY KEY,
    name                text,
    password_hash       text,
    profile_picture     text,
    channel_picture     text,
    preferred_theme     text,
    preferred_language  text,
    totp_secret         text,
    totp_enabled        boolean
);

-- Migration for existing deployments:
-- ALTER TABLE users ADD preferred_language text;

-- ══════════════════════════════════════════════════════════════════════
-- MEDIA
-- ══════════════════════════════════════════════════════════════════════

-- Primary media lookup by ID
CREATE TABLE IF NOT EXISTS media (
    id                  text PRIMARY KEY,
    name                text,
    description         text,  -- JSON string (was JSONB in PG)
    upload              bigint,
    owner               text,
    views               bigint,
    type                text,
    public              boolean,
    visibility          text,
    restricted_to_group text
);

-- Media by owner, sorted by upload time (for channel pages, studio)
CREATE TABLE IF NOT EXISTS media_by_owner (
    owner    text,
    upload   bigint,
    id       text,
    name     text,
    description text,
    views    bigint,
    type     text,
    public   boolean,
    visibility text,
    restricted_to_group text,
    PRIMARY KEY (owner, upload, id)
) WITH CLUSTERING ORDER BY (upload DESC, id ASC);

-- ══════════════════════════════════════════════════════════════════════
-- MEDIA CONCEPTS (upload processing pipeline)
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS media_concepts (
    id        text PRIMARY KEY,
    name      text,
    owner     text,
    type      text,
    processed boolean
);

-- Unprocessed concepts lookup (for the processor polling loop)
CREATE TABLE IF NOT EXISTS unprocessed_concepts (
    partition   int,  -- fixed partition key (always 0) for single-partition scan
    id          text,
    type        text,
    PRIMARY KEY (partition, id)
);

-- Concepts by owner (for studio concepts page)
CREATE TABLE IF NOT EXISTS media_concepts_by_owner (
    owner     text,
    id        text,
    name      text,
    type      text,
    processed boolean,
    PRIMARY KEY (owner, id)
);

-- ══════════════════════════════════════════════════════════════════════
-- VIEW COUNTS (counter table)
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS media_view_counts (
    id    text PRIMARY KEY,
    views counter
);

-- ══════════════════════════════════════════════════════════════════════
-- LIKES / DISLIKES
-- ══════════════════════════════════════════════════════════════════════

-- Per-user reaction (one row per user-media pair)
CREATE TABLE IF NOT EXISTS media_likes (
    media_id   text,
    user_login text,
    reaction   text,   -- 'like' or 'dislike'
    PRIMARY KEY (media_id, user_login)
);

-- ══════════════════════════════════════════════════════════════════════
-- COMMENTS
-- ══════════════════════════════════════════════════════════════════════

-- Comments per media, ordered by time descending
-- Using timeuuid for unique, time-ordered comment IDs
CREATE TABLE IF NOT EXISTS comments (
    media    text,
    time     bigint,
    id       bigint,  -- snowflake/timestamp-based ID generated in app
    user     text,
    text     text,    -- JSON string (was JSONB in PG)
    PRIMARY KEY (media, time, id)
) WITH CLUSTERING ORDER BY (time DESC, id DESC);

-- ══════════════════════════════════════════════════════════════════════
-- SUBSCRIPTIONS
-- ══════════════════════════════════════════════════════════════════════

-- Subscriptions: who does a user subscribe to?
CREATE TABLE IF NOT EXISTS subscriptions (
    subscriber text,
    target     text,
    PRIMARY KEY (subscriber, target)
);

-- Reverse index: who subscribes to a target? (for subscriber count)
CREATE TABLE IF NOT EXISTS subscribers_by_target (
    target     text,
    subscriber text,
    PRIMARY KEY (target, subscriber)
);

-- ══════════════════════════════════════════════════════════════════════
-- LISTS
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS lists (
    id                  text PRIMARY KEY,
    name                text,
    owner               text,
    public              boolean,
    visibility          text,
    restricted_to_group text,
    created             bigint
);

-- Lists by owner, sorted by creation time
CREATE TABLE IF NOT EXISTS lists_by_owner (
    owner    text,
    created  bigint,
    id       text,
    name     text,
    public   boolean,
    visibility text,
    restricted_to_group text,
    PRIMARY KEY (owner, created, id)
) WITH CLUSTERING ORDER BY (created DESC, id ASC);

-- List items (ordered by position within a list)
CREATE TABLE IF NOT EXISTS list_items (
    list_id   text,
    position  int,
    media_id  text,
    PRIMARY KEY (list_id, position)
);

-- Reverse lookup: which lists contain a given media?
CREATE TABLE IF NOT EXISTS list_items_by_media (
    media_id text,
    list_id  text,
    position int,
    PRIMARY KEY (media_id, list_id)
);

-- ══════════════════════════════════════════════════════════════════════
-- USER GROUPS
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS user_groups (
    id      text PRIMARY KEY,
    name    text,
    owner   text,
    created bigint
);

-- Groups owned by a user
CREATE TABLE IF NOT EXISTS user_groups_by_owner (
    owner   text,
    created bigint,
    id      text,
    name    text,
    PRIMARY KEY (owner, created, id)
) WITH CLUSTERING ORDER BY (created DESC, id ASC);

-- Group membership
CREATE TABLE IF NOT EXISTS user_group_members (
    group_id   text,
    user_login text,
    PRIMARY KEY (group_id, user_login)
);

-- Reverse: which groups is a user a member of?
CREATE TABLE IF NOT EXISTS user_groups_by_member (
    user_login text,
    group_id   text,
    PRIMARY KEY (user_login, group_id)
);

-- ══════════════════════════════════════════════════════════════════════
-- VIEW HISTORY (per-user watch history)
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS view_history (
    user_login text,
    viewed_at  bigint,
    media_id   text,
    PRIMARY KEY (user_login, viewed_at, media_id)
) WITH CLUSTERING ORDER BY (viewed_at DESC, media_id ASC);

-- ══════════════════════════════════════════════════════════════════════
-- WEBAUTHN CREDENTIALS
-- ══════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id              text PRIMARY KEY,
    user_login      text,
    credential_name text,
    passkey         text,   -- JSON string (was JSONB in PG)
    created         bigint
);

-- WebAuthn credentials by user
CREATE TABLE IF NOT EXISTS webauthn_credentials_by_user (
    user_login      text,
    created         bigint,
    id              text,
    credential_name text,
    passkey         text,
    PRIMARY KEY (user_login, created, id)
) WITH CLUSTERING ORDER BY (created ASC, id ASC);
