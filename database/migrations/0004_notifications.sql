-- Nook notifications.

CREATE TABLE notifications (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN (
                        'friend_request_received', 'friend_request_accepted',
                        'added_to_group', 'removed_from_group', 'group_updated',
                        'member_joined', 'member_left', 'message')),
    title           TEXT NOT NULL,
    body            TEXT NOT NULL DEFAULT '',
    actor_id        UUID REFERENCES users (id) ON DELETE SET NULL,
    conversation_id UUID REFERENCES conversations (id) ON DELETE CASCADE,
    message_id      UUID REFERENCES messages (id) ON DELETE SET NULL,
    data            JSONB NOT NULL DEFAULT '{}',
    read_at         TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX notifications_user_created_idx ON notifications (user_id, created_at DESC);
CREATE INDEX notifications_unread_idx ON notifications (user_id) WHERE read_at IS NULL;
