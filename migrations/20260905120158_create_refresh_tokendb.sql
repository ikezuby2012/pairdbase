-- Add migration script here
CREATE TABLE tbl_refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    user_id UUID NOT NULL,

    token_hash VARCHAR(128) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,

    -- Audit fields
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    created_by UUID,
    updated_by UUID,

    is_soft_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMPTZ,

    CONSTRAINT tbl_refresh_tokens_user_fk
        FOREIGN KEY (user_id)
        REFERENCES tbl_users(id)
        ON DELETE CASCADE,

    CONSTRAINT tbl_refresh_tokens_token_hash_key
        UNIQUE (token_hash)
);