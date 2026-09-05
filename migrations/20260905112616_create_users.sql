-- Add migration script here
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    organization_id UUID NOT NULL,

    email VARCHAR(320) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    avatar_url TEXT,

    password_hash TEXT,

    role VARCHAR(50) NOT NULL DEFAULT 'user',

    is_verified BOOLEAN NOT NULL DEFAULT FALSE,

    last_login_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,

    updated_by UUID,

    is_soft_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMPTZ,

    CONSTRAINT users_organization_id_email_key
        UNIQUE (organization_id, email)
);