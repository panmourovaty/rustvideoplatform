use scylla::statement::prepared::PreparedStatement;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use std::sync::Arc;

/// Wrapper around a ScyllaDB session with pre-prepared statements for all queries.
#[derive(Clone)]
pub struct ScyllaDb {
    pub session: Arc<Session>,

    // --- Users ---
    pub get_user_by_login: PreparedStatement,
    pub get_user_password: PreparedStatement,
    pub get_user_totp_enabled: PreparedStatement,
    pub get_user_totp_secret: PreparedStatement,
    pub update_user_name: PreparedStatement,
    pub update_user_password: PreparedStatement,
    pub update_user_profile_picture: PreparedStatement,
    pub update_user_channel_picture: PreparedStatement,
    pub update_user_theme: PreparedStatement,
    pub get_user_theme: PreparedStatement,
    pub update_user_language: PreparedStatement,
    pub get_user_language: PreparedStatement,
    pub get_user_profile_picture: PreparedStatement,
    pub get_user_channel_picture: PreparedStatement,
    pub update_user_totp: PreparedStatement,
    pub disable_user_totp: PreparedStatement,

    // --- Media ---
    pub get_media_by_id: PreparedStatement,
    pub get_media_owner: PreparedStatement,
    pub get_media_basic: PreparedStatement,
    pub insert_media: PreparedStatement,
    pub update_media_name_desc: PreparedStatement,
    pub update_media_permissions: PreparedStatement,
    pub delete_media: PreparedStatement,
    pub get_media_by_owner: PreparedStatement,
    pub get_media_by_owner_pictures: PreparedStatement,
    pub insert_media_by_owner: PreparedStatement,
    pub delete_media_by_owner: PreparedStatement,
    pub get_media_description: PreparedStatement,

    // --- Media View Counts ---
    pub increment_view_count: PreparedStatement,
    pub get_view_count: PreparedStatement,

    // --- Comments ---
    pub get_comments: PreparedStatement,
    pub get_comment_text: PreparedStatement,
    pub insert_comment: PreparedStatement,

    // --- Media Likes ---
    pub get_user_reaction: PreparedStatement,
    pub upsert_reaction: PreparedStatement,
    pub delete_reaction: PreparedStatement,
    pub get_reactions_for_media: PreparedStatement,

    // --- Subscriptions ---
    pub insert_subscription: PreparedStatement,
    pub insert_subscriber_by_target: PreparedStatement,
    pub delete_subscription: PreparedStatement,
    pub delete_subscriber_by_target: PreparedStatement,
    pub is_subscribed: PreparedStatement,
    pub get_subscriptions: PreparedStatement,
    pub count_subscribers: PreparedStatement,

    // --- Media Concepts ---
    pub insert_concept: PreparedStatement,
    pub insert_concept_by_owner: PreparedStatement,
    pub insert_unprocessed_concept: PreparedStatement,
    pub get_concept: PreparedStatement,
    pub get_concepts_by_owner: PreparedStatement,
    pub delete_concept: PreparedStatement,
    pub delete_concept_by_owner: PreparedStatement,
    pub delete_unprocessed_concept: PreparedStatement,
    pub mark_concept_processed: PreparedStatement,
    pub mark_concept_processed_by_owner: PreparedStatement,
    pub get_unprocessed_concepts: PreparedStatement,

    // --- Lists ---
    pub get_list_by_id: PreparedStatement,
    pub get_list_owner: PreparedStatement,
    pub insert_list: PreparedStatement,
    pub insert_list_by_owner: PreparedStatement,
    pub delete_list: PreparedStatement,
    pub delete_list_by_owner: PreparedStatement,
    pub get_lists_by_owner: PreparedStatement,

    // --- List Items ---
    pub get_list_items: PreparedStatement,
    pub insert_list_item: PreparedStatement,
    pub insert_list_item_by_media: PreparedStatement,
    pub delete_list_item: PreparedStatement,
    pub delete_list_item_by_media: PreparedStatement,
    pub get_list_items_by_media: PreparedStatement,
    pub get_max_list_position: PreparedStatement,
    pub count_list_items: PreparedStatement,
    pub delete_all_list_items: PreparedStatement,

    // --- User Groups ---
    pub insert_group: PreparedStatement,
    pub insert_group_by_owner: PreparedStatement,
    pub get_group_by_id: PreparedStatement,
    pub delete_group: PreparedStatement,
    pub delete_group_by_owner: PreparedStatement,
    pub get_groups_by_owner: PreparedStatement,

    // --- User Group Members ---
    pub insert_group_member: PreparedStatement,
    pub insert_group_by_member: PreparedStatement,
    pub delete_group_member: PreparedStatement,
    pub delete_group_by_member: PreparedStatement,
    pub get_group_members: PreparedStatement,
    pub get_groups_for_user: PreparedStatement,
    pub check_user_exists: PreparedStatement,

    // --- WebAuthn ---
    pub get_webauthn_creds_by_user: PreparedStatement,
    pub insert_webauthn_cred: PreparedStatement,
    pub insert_webauthn_cred_by_user: PreparedStatement,
    pub delete_webauthn_cred: PreparedStatement,
    pub delete_webauthn_cred_by_user: PreparedStatement,
    pub update_webauthn_passkey: PreparedStatement,
    pub get_webauthn_cred_by_id: PreparedStatement,

    // --- View History ---
    pub insert_view_history: PreparedStatement,
    pub get_view_history: PreparedStatement,

    // --- Batch update helpers for groups ---
    pub get_media_by_group: PreparedStatement,
    pub get_lists_by_group: PreparedStatement,
}

impl ScyllaDb {
    pub async fn connect(nodes: &[String], keyspace: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let session = SessionBuilder::new()
            .known_nodes(nodes)
            .use_keyspace(keyspace, false)
            .build()
            .await?;
        let session = Arc::new(session);

        let db = ScyllaDb {
            // --- Users ---
            get_user_by_login: session.prepare("SELECT name, profile_picture FROM users WHERE login = ?").await?,
            get_user_password: session.prepare("SELECT password_hash FROM users WHERE login = ?").await?,
            get_user_totp_enabled: session.prepare("SELECT totp_enabled FROM users WHERE login = ?").await?,
            get_user_totp_secret: session.prepare("SELECT totp_secret FROM users WHERE login = ? AND totp_enabled = true ALLOW FILTERING").await?,
            update_user_name: session.prepare("UPDATE users SET name = ? WHERE login = ?").await?,
            update_user_password: session.prepare("UPDATE users SET password_hash = ? WHERE login = ?").await?,
            update_user_profile_picture: session.prepare("UPDATE users SET profile_picture = ? WHERE login = ?").await?,
            update_user_channel_picture: session.prepare("UPDATE users SET channel_picture = ? WHERE login = ?").await?,
            update_user_theme: session.prepare("UPDATE users SET preferred_theme = ? WHERE login = ?").await?,
            get_user_theme: session.prepare("SELECT preferred_theme FROM users WHERE login = ?").await?,
            update_user_language: session.prepare("UPDATE users SET preferred_language = ? WHERE login = ?").await?,
            get_user_language: session.prepare("SELECT preferred_language FROM users WHERE login = ?").await?,
            get_user_profile_picture: session.prepare("SELECT profile_picture FROM users WHERE login = ?").await?,
            get_user_channel_picture: session.prepare("SELECT channel_picture FROM users WHERE login = ?").await?,
            update_user_totp: session.prepare("UPDATE users SET totp_secret = ?, totp_enabled = true WHERE login = ?").await?,
            disable_user_totp: session.prepare("UPDATE users SET totp_secret = null, totp_enabled = false WHERE login = ?").await?,

            // --- Media ---
            get_media_by_id: session.prepare("SELECT id, name, description, upload, owner, views, type, visibility, restricted_to_group FROM media WHERE id = ?").await?,
            get_media_owner: session.prepare("SELECT owner FROM media WHERE id = ?").await?,
            get_media_basic: session.prepare("SELECT id, name, owner, visibility, restricted_to_group, type FROM media WHERE id = ?").await?,
            insert_media: session.prepare("INSERT INTO media (id, name, description, upload, owner, views, type, public, visibility, restricted_to_group) VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?, ?)").await?,
            update_media_name_desc: session.prepare("UPDATE media SET name = ?, description = ? WHERE id = ?").await?,
            update_media_permissions: session.prepare("UPDATE media SET public = ?, visibility = ?, restricted_to_group = ? WHERE id = ?").await?,
            delete_media: session.prepare("DELETE FROM media WHERE id = ?").await?,
            get_media_by_owner: session.prepare("SELECT id, name, description, views, type, upload, visibility, restricted_to_group FROM media_by_owner WHERE owner = ? LIMIT ? ").await?,
            get_media_by_owner_pictures: session.prepare("SELECT id, name, visibility FROM media_by_owner WHERE owner = ? LIMIT 1000").await?,
            insert_media_by_owner: session.prepare("INSERT INTO media_by_owner (owner, upload, id, name, description, views, type, public, visibility, restricted_to_group) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)").await?,
            delete_media_by_owner: session.prepare("DELETE FROM media_by_owner WHERE owner = ? AND upload = ? AND id = ?").await?,
            get_media_description: session.prepare("SELECT description FROM media WHERE id = ?").await?,

            // --- Media View Counts ---
            increment_view_count: session.prepare("UPDATE media_view_counts SET views = views + 1 WHERE id = ?").await?,
            get_view_count: session.prepare("SELECT views FROM media_view_counts WHERE id = ?").await?,

            // --- Comments ---
            get_comments: session.prepare("SELECT id, user, text, time FROM comments WHERE media = ? ORDER BY time DESC, id DESC LIMIT ?").await?,
            get_comment_text: session.prepare("SELECT text FROM comments WHERE media = ? AND time = ? AND id = ?").await?,
            insert_comment: session.prepare("INSERT INTO comments (media, time, id, user, text) VALUES (?, ?, ?, ?, ?)").await?,

            // --- Media Likes ---
            get_user_reaction: session.prepare("SELECT reaction FROM media_likes WHERE media_id = ? AND user_login = ?").await?,
            upsert_reaction: session.prepare("INSERT INTO media_likes (media_id, user_login, reaction) VALUES (?, ?, ?)").await?,
            delete_reaction: session.prepare("DELETE FROM media_likes WHERE media_id = ? AND user_login = ?").await?,
            get_reactions_for_media: session.prepare("SELECT user_login, reaction FROM media_likes WHERE media_id = ?").await?,

            // --- Subscriptions ---
            insert_subscription: session.prepare("INSERT INTO subscriptions (subscriber, target) VALUES (?, ?)").await?,
            insert_subscriber_by_target: session.prepare("INSERT INTO subscribers_by_target (target, subscriber) VALUES (?, ?)").await?,
            delete_subscription: session.prepare("DELETE FROM subscriptions WHERE subscriber = ? AND target = ?").await?,
            delete_subscriber_by_target: session.prepare("DELETE FROM subscribers_by_target WHERE target = ? AND subscriber = ?").await?,
            is_subscribed: session.prepare("SELECT target FROM subscriptions WHERE subscriber = ? AND target = ?").await?,
            get_subscriptions: session.prepare("SELECT target FROM subscriptions WHERE subscriber = ?").await?,
            count_subscribers: session.prepare("SELECT subscriber FROM subscribers_by_target WHERE target = ?").await?,

            // --- Media Concepts ---
            insert_concept: session.prepare("INSERT INTO media_concepts (id, name, owner, type, processed) VALUES (?, ?, ?, ?, false)").await?,
            insert_concept_by_owner: session.prepare("INSERT INTO media_concepts_by_owner (owner, id, name, type, processed) VALUES (?, ?, ?, ?, false)").await?,
            insert_unprocessed_concept: session.prepare("INSERT INTO unprocessed_concepts (partition, id, type) VALUES (0, ?, ?)").await?,
            get_concept: session.prepare("SELECT id, name, type, processed FROM media_concepts WHERE id = ?").await?,
            get_concepts_by_owner: session.prepare("SELECT id, name, type, processed FROM media_concepts_by_owner WHERE owner = ?").await?,
            delete_concept: session.prepare("DELETE FROM media_concepts WHERE id = ?").await?,
            delete_concept_by_owner: session.prepare("DELETE FROM media_concepts_by_owner WHERE owner = ? AND id = ?").await?,
            delete_unprocessed_concept: session.prepare("DELETE FROM unprocessed_concepts WHERE partition = 0 AND id = ?").await?,
            mark_concept_processed: session.prepare("UPDATE media_concepts SET processed = true WHERE id = ?").await?,
            mark_concept_processed_by_owner: session.prepare("UPDATE media_concepts_by_owner SET processed = true WHERE owner = ? AND id = ?").await?,
            get_unprocessed_concepts: session.prepare("SELECT id, type FROM unprocessed_concepts WHERE partition = 0").await?,

            // --- Lists ---
            get_list_by_id: session.prepare("SELECT id, name, owner, visibility, restricted_to_group, created FROM lists WHERE id = ?").await?,
            get_list_owner: session.prepare("SELECT owner FROM lists WHERE id = ?").await?,
            insert_list: session.prepare("INSERT INTO lists (id, name, owner, public, visibility, restricted_to_group, created) VALUES (?, ?, ?, ?, ?, ?, ?)").await?,
            insert_list_by_owner: session.prepare("INSERT INTO lists_by_owner (owner, created, id, name, public, visibility, restricted_to_group) VALUES (?, ?, ?, ?, ?, ?, ?)").await?,
            delete_list: session.prepare("DELETE FROM lists WHERE id = ?").await?,
            delete_list_by_owner: session.prepare("DELETE FROM lists_by_owner WHERE owner = ? AND created = ? AND id = ?").await?,
            get_lists_by_owner: session.prepare("SELECT id, name, visibility, restricted_to_group, created FROM lists_by_owner WHERE owner = ? LIMIT ?").await?,

            // --- List Items ---
            get_list_items: session.prepare("SELECT media_id, position FROM list_items WHERE list_id = ? ORDER BY position ASC LIMIT ?").await?,
            insert_list_item: session.prepare("INSERT INTO list_items (list_id, position, media_id) VALUES (?, ?, ?)").await?,
            insert_list_item_by_media: session.prepare("INSERT INTO list_items_by_media (media_id, list_id, position) VALUES (?, ?, ?)").await?,
            delete_list_item: session.prepare("DELETE FROM list_items WHERE list_id = ? AND position = ?").await?,
            delete_list_item_by_media: session.prepare("DELETE FROM list_items_by_media WHERE media_id = ? AND list_id = ?").await?,
            get_list_items_by_media: session.prepare("SELECT list_id, position FROM list_items_by_media WHERE media_id = ?").await?,
            get_max_list_position: session.prepare("SELECT position FROM list_items WHERE list_id = ? ORDER BY position DESC LIMIT 1").await?,
            count_list_items: session.prepare("SELECT position FROM list_items WHERE list_id = ?").await?,
            delete_all_list_items: session.prepare("SELECT position, media_id FROM list_items WHERE list_id = ?").await?,

            // --- User Groups ---
            insert_group: session.prepare("INSERT INTO user_groups (id, name, owner, created) VALUES (?, ?, ?, ?)").await?,
            insert_group_by_owner: session.prepare("INSERT INTO user_groups_by_owner (owner, created, id, name) VALUES (?, ?, ?, ?)").await?,
            get_group_by_id: session.prepare("SELECT id, name, owner FROM user_groups WHERE id = ?").await?,
            delete_group: session.prepare("DELETE FROM user_groups WHERE id = ?").await?,
            delete_group_by_owner: session.prepare("DELETE FROM user_groups_by_owner WHERE owner = ? AND created = ? AND id = ?").await?,
            get_groups_by_owner: session.prepare("SELECT id, name, created FROM user_groups_by_owner WHERE owner = ?").await?,

            // --- User Group Members ---
            insert_group_member: session.prepare("INSERT INTO user_group_members (group_id, user_login) VALUES (?, ?)").await?,
            insert_group_by_member: session.prepare("INSERT INTO user_groups_by_member (user_login, group_id) VALUES (?, ?)").await?,
            delete_group_member: session.prepare("DELETE FROM user_group_members WHERE group_id = ? AND user_login = ?").await?,
            delete_group_by_member: session.prepare("DELETE FROM user_groups_by_member WHERE user_login = ? AND group_id = ?").await?,
            get_group_members: session.prepare("SELECT user_login FROM user_group_members WHERE group_id = ?").await?,
            get_groups_for_user: session.prepare("SELECT group_id FROM user_groups_by_member WHERE user_login = ?").await?,
            check_user_exists: session.prepare("SELECT login FROM users WHERE login = ?").await?,

            // --- WebAuthn ---
            get_webauthn_creds_by_user: session.prepare("SELECT id, credential_name, passkey, created FROM webauthn_credentials_by_user WHERE user_login = ?").await?,
            insert_webauthn_cred: session.prepare("INSERT INTO webauthn_credentials (id, user_login, credential_name, passkey, created) VALUES (?, ?, ?, ?, ?)").await?,
            insert_webauthn_cred_by_user: session.prepare("INSERT INTO webauthn_credentials_by_user (user_login, created, id, credential_name, passkey) VALUES (?, ?, ?, ?, ?)").await?,
            delete_webauthn_cred: session.prepare("DELETE FROM webauthn_credentials WHERE id = ?").await?,
            delete_webauthn_cred_by_user: session.prepare("DELETE FROM webauthn_credentials_by_user WHERE user_login = ? AND created = ? AND id = ?").await?,
            update_webauthn_passkey: session.prepare("UPDATE webauthn_credentials SET passkey = ? WHERE id = ?").await?,
            get_webauthn_cred_by_id: session.prepare("SELECT id, user_login, credential_name, passkey, created FROM webauthn_credentials WHERE id = ?").await?,

            // --- View History ---
            insert_view_history: session.prepare("INSERT INTO view_history (user_login, viewed_at, media_id) VALUES (?, ?, ?)").await?,
            get_view_history: session.prepare("SELECT media_id, viewed_at FROM view_history WHERE user_login = ? LIMIT ?").await?,

            // --- Batch update helpers for groups ---
            get_media_by_group: session.prepare("SELECT id, owner, upload FROM media WHERE restricted_to_group = ? ALLOW FILTERING").await?,
            get_lists_by_group: session.prepare("SELECT id, owner, created FROM lists WHERE restricted_to_group = ? ALLOW FILTERING").await?,

            session,
        };

        Ok(db)
    }
}
