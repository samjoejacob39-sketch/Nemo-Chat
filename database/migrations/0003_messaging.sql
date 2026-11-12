-- Nook messaging: conversations, membership, messages, reactions, receipts.

CREATE TABLE conversations (
    id                   UUID PRIMARY KEY,
    kind                 TEXT NOT NULL CHECK (kind IN ('private', 'group')),
    title                TEXT,
    description          TEXT NOT NULL DEFAULT '',
    avatar_attachment_id UUID REFERENCES attachments (id) ON DELETE SET NULL,
    dm_key               TEXT UNIQUE, -- "uuidA|uuidB" sorted, for private chats
    created_by           UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_message_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT conversations_group_title CHECK (kind <> 'group' OR title IS NOT NULL)
);

CREATE INDEX conversations_last_message_idx ON conversations (last_message_at DESC);

CREATE TABLE messages (
    id              UUID PRIMARY KEY, -- app generates UUIDv7 (time-ordered)
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    sender_id       UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('text', 'image', 'system')),
    body            TEXT NOT NULL DEFAULT '',
    attachment_id   UUID REFERENCES attachments (id) ON DELETE SET NULL,
    reply_to_id     UUID REFERENCES messages (id) ON DELETE SET NULL,
    pinned_at       TIMESTAMPTZ,
    edited_at       TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX messages_conversation_id_idx ON messages (conversation_id, id DESC);
CREATE INDEX messages_conversation_created_idx ON messages (conversation_id, created_at DESC);

ALTER TABLE reports
    ADD CONSTRAINT reports_conversation_fk
        FOREIGN KEY (conversation_id) REFERENCES conversations (id) ON DELETE SET NULL,
    ADD CONSTRAINT reports_message_fk
        FOREIGN KEY (message_id) REFERENCES messages (id) ON DELETE SET NULL;

CREATE TABLE conversation_members (
    id                   UUID PRIMARY KEY,
    conversation_id      UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    user_id              UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role                 TEXT NOT NULL DEFAULT 'member'
                         CHECK (role IN ('owner', 'admin', 'member')),
    joined_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    muted_at             TIMESTAMPTZ,
    pinned_at            TIMESTAMPTZ,
    last_read_message_id UUID REFERENCES messages (id) ON DELETE SET NULL,
    CONSTRAINT conversation_members_unique UNIQUE (conversation_id, user_id)
);

CREATE INDEX conversation_members_user_idx ON conversation_members (user_id);

CREATE TABLE message_hides (
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    hidden_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, message_id)
);

CREATE TABLE message_reactions (
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    emoji      TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (message_id, user_id)
);

CREATE TABLE read_receipts (
    id                   UUID PRIMARY KEY,
    conversation_id      UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    user_id              UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    last_read_message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    read_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT read_receipts_unique UNIQUE (conversation_id, user_id)
);
