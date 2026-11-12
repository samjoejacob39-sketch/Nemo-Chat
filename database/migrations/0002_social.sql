-- Nook social graph: friend requests, friendships, blocks, reports.

CREATE TABLE friend_requests (
    id           UUID PRIMARY KEY,
    sender_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    receiver_id  UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    status       TEXT NOT NULL DEFAULT 'pending'
                 CHECK (status IN ('pending', 'accepted', 'declined', 'cancelled')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    responded_at TIMESTAMPTZ,
    CONSTRAINT friend_requests_not_self CHECK (sender_id <> receiver_id)
);

-- Only one active request per direction; declined/cancelled rows allow retry.
CREATE UNIQUE INDEX friend_requests_active_idx
    ON friend_requests (sender_id, receiver_id) WHERE status = 'pending';

CREATE INDEX friend_requests_receiver_pending_idx
    ON friend_requests (receiver_id) WHERE status = 'pending';

CREATE TABLE friendships (
    id         UUID PRIMARY KEY,
    user_id_a  UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_id_b  UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT friendships_canonical CHECK (user_id_a < user_id_b),
    CONSTRAINT friendships_not_self CHECK (user_id_a <> user_id_b)
);

CREATE UNIQUE INDEX friendships_pair_idx ON friendships (user_id_a, user_id_b);
CREATE INDEX friendships_a_idx ON friendships (user_id_a);
CREATE INDEX friendships_b_idx ON friendships (user_id_b);

CREATE TABLE blocked_users (
    blocker_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    blocked_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT blocked_users_not_self CHECK (blocker_id <> blocked_id),
    PRIMARY KEY (blocker_id, blocked_id)
);

CREATE INDEX blocked_users_blocked_idx ON blocked_users (blocked_id);

CREATE TABLE reports (
    id              UUID PRIMARY KEY,
    reporter_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    target_user_id  UUID REFERENCES users (id) ON DELETE SET NULL,
    conversation_id UUID, -- FK added in 0003 when conversations exist
    message_id      UUID, -- FK added in 0003
    reason          TEXT NOT NULL,
    details         TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reports_reporter_idx ON reports (reporter_id);
