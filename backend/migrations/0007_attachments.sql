CREATE TABLE plugin_memory_attachments (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES plugin_memory_sources(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    owner_id TEXT NOT NULL,
    filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 255),
    content_type TEXT NOT NULL CHECK (content_type IN ('image/png','image/jpeg','image/gif','image/webp')),
    size_bytes BIGINT NOT NULL CHECK (size_bytes BETWEEN 1 AND 700000),
    sha256 TEXT NOT NULL CHECK (length(sha256)=64),
    ciphertext BYTEA NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX plugin_memory_attachment_source ON plugin_memory_attachments(source_id,created_at);
