-- Add migration script here
ALTER TABLE tbl_refresh_tokens
ADD COLUMN revoked_at TIMESTAMPTZ;