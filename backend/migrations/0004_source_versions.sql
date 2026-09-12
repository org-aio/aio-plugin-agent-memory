CREATE TABLE plugin_memory_clarifications (
    source_id TEXT NOT NULL REFERENCES plugin_memory_sources(id),
    explanation_id TEXT NOT NULL REFERENCES plugin_memory_sources(id),
    source_version BIGINT NOT NULL,
    PRIMARY KEY (source_id,explanation_id)
);
CREATE TABLE plugin_memory_revision_sources (
    revision_id TEXT PRIMARY KEY REFERENCES plugin_memory_revisions(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES plugin_memory_sources(id),
    source_version BIGINT NOT NULL
);
