-- Nook core: users, attachments, auth sessions, recovery codes, settings,
-- terms acceptances.

CREATE TABLE users (
    id            UUID PRIMARY KEY,
    username      TEXT NOT NULL CHECK (username = lower(username)),
    display_name  TEXT NOT NULL,
    bio           TEXT NOT NULL DEFAULT '',
    password_hash TEXT NOT NULL,
    last_seen_at  TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Case-insensitive uniqueness (belt & braces; the app always stores lowercase).
CREATE UNIQUE INDEX users_username_lower_idx ON users (lower(username));

CREATE TABLE attachments (
    id           UUID PRIMARY KEY,
    owner_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind         TEXT NOT NULL CHECK (kind IN ('image', 'avatar')),
    mime         TEXT NOT NULL,
    byte_size    BIGINT NOT NULL,
    width        INTEGER,
    height       INTEGER,
    storage_path TEXT NOT NULL,
    sha256       TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE users
    ADD COLUMN avatar_attachment_id UUID REFERENCES attachments (id) ON DELETE SET NULL;

CREATE TABLE sessions (
    id           UUID PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    device       TEXT NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);

CREATE INDEX sessions_user_idx ON sessions (user_id);

CREATE TABLE recovery_codes (
    id        UUID PRIMARY KEY,
    user_id   UUID NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    used_at   TIMESTAMPTZ
);

CREATE TABLE user_settings (
    user_id                UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    online_visibility      TEXT NOT NULL DEFAULT 'friends'
                           CHECK (online_visibility IN ('everyone', 'friends', 'nobody')),
    last_seen_visibility   TEXT NOT NULL DEFAULT 'friends'
                           CHECK (last_seen_visibility IN ('everyone', 'friends', 'nobody')),
    read_receipts          BOOLEAN NOT NULL DEFAULT TRUE,
    searchability          TEXT NOT NULL DEFAULT 'everyone'
                           CHECK (searchability IN ('everyone', 'friends', 'nobody')),
    theme                  TEXT NOT NULL DEFAULT 'system'
                           CHECK (theme IN ('system', 'light', 'dark')),
    notify_friend_requests BOOLEAN NOT NULL DEFAULT TRUE,
    notify_messages        BOOLEAN NOT NULL DEFAULT TRUE,
    notify_group_events    BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE terms_acceptances (
    id             UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    policy_version TEXT NOT NULL,
    accepted_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX terms_acceptances_user_version_idx
    ON terms_acceptances (user_id, policy_version);
