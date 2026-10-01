CREATE TABLE plugin_memory_ssh_hosts (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    alias TEXT NOT NULL CHECK (alias ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,62}$'),
    hostname TEXT NOT NULL CHECK (length(hostname) BETWEEN 1 AND 253),
    username TEXT NOT NULL CHECK (username ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$'),
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    identity_file TEXT NOT NULL CHECK (identity_file ~ '^id_(ed25519|rsa|ecdsa)$'),
    device_id TEXT NOT NULL,
    device_label TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','applied','verified','error')),
    last_error TEXT,
    version BIGINT NOT NULL DEFAULT 1,
    updated_at BIGINT NOT NULL,
    UNIQUE (tenant_id,user_id,alias)
);
CREATE INDEX plugin_memory_ssh_hosts_owner ON plugin_memory_ssh_hosts(tenant_id,user_id,updated_at DESC);
