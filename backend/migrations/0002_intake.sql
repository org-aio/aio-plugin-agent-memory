CREATE TABLE plugin_memory_spaces (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    personal_owner TEXT UNIQUE,
    model_binding TEXT,
    created_by TEXT NOT NULL
);
CREATE TABLE plugin_memory_members (
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    user_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('OWNER','EDITOR','READER')),
    PRIMARY KEY (space_id,user_id)
);
CREATE TABLE plugin_memory_ownership (
    node_id TEXT PRIMARY KEY REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    created_by TEXT NOT NULL,
    author_type TEXT NOT NULL DEFAULT 'human' CHECK (author_type IN ('human','model','source'))
);
CREATE TABLE plugin_memory_sources (
    id TEXT PRIMARY KEY REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    created_by TEXT NOT NULL,
    request_id TEXT NOT NULL,
    ciphertext BYTEA NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','quarantined','processing','complete','conflict','failed','deleted')),
    origin TEXT NOT NULL,
    reference TEXT NOT NULL DEFAULT '',
    error TEXT,
    updated_at BIGINT NOT NULL,
    UNIQUE (space_id,created_by,request_id)
);
CREATE TABLE plugin_memory_secrets (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES plugin_memory_sources(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    label TEXT NOT NULL,
    ciphertext BYTEA NOT NULL,
    owner_id TEXT NOT NULL
);
CREATE TABLE plugin_memory_secret_grants (
    secret_id TEXT NOT NULL REFERENCES plugin_memory_secrets(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    can_reveal BOOLEAN NOT NULL,
    can_manage BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (secret_id,user_id)
);
CREATE TABLE plugin_memory_tasks (
    id TEXT PRIMARY KEY REFERENCES plugin_memory_sources(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    actor_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending','running','complete','failed','cancelled','conflict')),
    attempts BIGINT NOT NULL DEFAULT 0,
    available_at BIGINT NOT NULL,
    lease TEXT,
    lease_until BIGINT,
    worker_id TEXT,
    result JSONB,
    error TEXT
);
CREATE TABLE plugin_memory_revisions (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    version BIGINT NOT NULL,
    draft JSONB NOT NULL,
    author_id TEXT NOT NULL,
    author_type TEXT NOT NULL,
    source_id TEXT REFERENCES plugin_memory_sources(id),
    updated_at BIGINT NOT NULL,
    UNIQUE (node_id,version)
);
CREATE TABLE plugin_memory_evidence (
    node_id TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES plugin_memory_sources(id),
    PRIMARY KEY (node_id,source_id)
);
CREATE TABLE plugin_memory_credential_links (
    node_id TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    secret_id TEXT NOT NULL REFERENCES plugin_memory_secrets(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id,secret_id)
);
CREATE INDEX plugin_memory_space_nodes ON plugin_memory_ownership(space_id,node_id);
CREATE INDEX plugin_memory_task_queue ON plugin_memory_tasks(space_id,state,available_at);
