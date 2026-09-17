-- Add migration script here
CREATE TABLE tbl_db_connections (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    organization_id     UUID NOT NULL,
    workspace_id        UUID NOT NULL,

    name                VARCHAR(255) NOT NULL,
    db_type             VARCHAR(50) NOT NULL,

    -- Database connection
    host                VARCHAR(255),
    port                INTEGER,
    database_name       VARCHAR(255),
    username            VARCHAR(255),

    -- Encrypted database secret
    password            BYTEA,

    -- SSL
    ssl_mode            VARCHAR(50) NOT NULL DEFAULT 'prefer',

    -- Encrypted SSL certificates/keys
    ssl_ca_cert         BYTEA,
    ssl_client_cert     BYTEA,
    ssl_client_key      BYTEA,

    -- SSH
    ssh_enabled         BOOLEAN NOT NULL DEFAULT FALSE,
    ssh_host            VARCHAR(255),
    ssh_port            INTEGER DEFAULT 22,
    ssh_user            VARCHAR(255),

    -- Encrypted SSH private key
    ssh_private_key     BYTEA,

    -- Connection behaviour
    read_only           BOOLEAN NOT NULL DEFAULT FALSE,
    color               VARCHAR(20),

    -- Connection pool
    pool_min            INTEGER NOT NULL DEFAULT 1,
    pool_max            INTEGER NOT NULL DEFAULT 10,

    -- Audit
    created_by          UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    updated_by          UUID,
    updated_at          TIMESTAMPTZ,

    is_soft_deleted     BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_by          UUID,
    deleted_at          TIMESTAMPTZ,

    -- Validation
    CONSTRAINT tbl_db_connections_port_chk
        CHECK (
            port IS NULL
            OR port BETWEEN 1 AND 65535
        ),

    CONSTRAINT tbl_db_connections_ssh_port_chk
        CHECK (
            ssh_port IS NULL
            OR ssh_port BETWEEN 1 AND 65535
        ),

    CONSTRAINT tbl_db_connections_pool_min_chk
        CHECK (pool_min >= 0),

    CONSTRAINT tbl_db_connections_pool_max_chk
        CHECK (pool_max >= pool_min),

    CONSTRAINT tbl_db_connections_ssh_config_chk
        CHECK (
            ssh_enabled = FALSE
            OR (
                ssh_host IS NOT NULL
                AND ssh_port IS NOT NULL
                AND ssh_user IS NOT NULL
                AND ssh_private_key IS NOT NULL
            )
        )
);

-- Workspace lookup
CREATE INDEX idx_tbl_db_connections_workspace
    ON tbl_db_connections (workspace_id)
    WHERE is_soft_deleted = FALSE;

-- Organization lookup
CREATE INDEX idx_tbl_db_connections_organization
    ON tbl_db_connections (organization_id)
    WHERE is_soft_deleted = FALSE;

-- Workspace + connection name
CREATE INDEX idx_tbl_db_connections_workspace_name
    ON tbl_db_connections (workspace_id, name)
    WHERE is_soft_deleted = FALSE;