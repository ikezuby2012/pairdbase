-- Add migration script here
CREATE TABLE tbl_oauth_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    state VARCHAR(255) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    code_verifier VARCHAR(255) NOT NULL,
    redirect_to TEXT,

    expires_at TIMESTAMPTZ NOT NULL,

    -- Audit fields
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,

    updated_at TIMESTAMPTZ,
    updated_by UUID,

    is_soft_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMPTZ,

    CONSTRAINT tbl_oauth_states_state_key
        UNIQUE (state)
);