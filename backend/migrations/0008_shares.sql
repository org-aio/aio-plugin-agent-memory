-- 一条笔记最多一个当前分享链接；重新生成会替换令牌并立即让旧链接失效。
CREATE TABLE plugin_memory_shares (
    source_id TEXT PRIMARY KEY REFERENCES plugin_memory_sources(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE CHECK (length(token) = 64),
    space_id TEXT NOT NULL REFERENCES plugin_memory_spaces(id),
    created_by TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    CHECK (expires_at > created_at)
);
CREATE INDEX plugin_memory_share_expiry ON plugin_memory_shares(expires_at);
