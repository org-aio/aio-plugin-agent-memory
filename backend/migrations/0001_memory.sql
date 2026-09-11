CREATE TABLE plugin_memory_nodes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 160),
    kind TEXT NOT NULL CHECK (kind IN ('NOTE', 'CONCEPT', 'PERSON', 'EVENT', 'SOURCE', 'PROJECT')),
    content TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    tags JSONB NOT NULL DEFAULT '[]',
    version BIGINT NOT NULL DEFAULT 1,
    updated_at BIGINT NOT NULL
);
CREATE TABLE plugin_memory_edges (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    target TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK (length(relation) BETWEEN 1 AND 48),
    evidence TEXT NOT NULL DEFAULT '',
    CHECK (source <> target),
    UNIQUE (source, target, relation)
);
