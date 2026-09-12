CREATE TABLE plugin_memory_aliases (
    node_id TEXT NOT NULL REFERENCES plugin_memory_nodes(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    PRIMARY KEY (node_id,alias)
);
