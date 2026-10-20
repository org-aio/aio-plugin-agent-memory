-- 0006 已在生产执行；现有安装通过本迁移补齐代理字段，空字符串表示直连。
ALTER TABLE plugin_memory_ssh_hosts
    ADD COLUMN proxy_jump TEXT NOT NULL DEFAULT '',
    ADD COLUMN proxy_command TEXT NOT NULL DEFAULT '';
-- 设备上原本已存在、仅登记到记忆里的主机使用 registered，不暗示需要重写。
ALTER TABLE plugin_memory_ssh_hosts
    DROP CONSTRAINT IF EXISTS plugin_memory_ssh_hosts_status_check;
ALTER TABLE plugin_memory_ssh_hosts
    ADD CONSTRAINT plugin_memory_ssh_hosts_status_check
    CHECK (status IN ('draft','registered','applied','verified','error'));
ALTER TABLE plugin_memory_ssh_hosts
    ADD CONSTRAINT plugin_memory_ssh_hosts_proxy_check
    CHECK (length(proxy_jump) <= 512 AND proxy_jump !~ '[[:cntrl:]]'
        AND length(proxy_command) <= 512 AND proxy_command !~ '[[:cntrl:]]'
        AND (proxy_jump = '' OR proxy_command = ''));
-- 现有私钥可能使用自定义文件名（如 nexus-deploy），仍限制为 ~/.ssh 下的普通文件名。
ALTER TABLE plugin_memory_ssh_hosts
    DROP CONSTRAINT IF EXISTS plugin_memory_ssh_hosts_identity_file_check;
ALTER TABLE plugin_memory_ssh_hosts
    ADD CONSTRAINT plugin_memory_ssh_hosts_identity_file_check
    CHECK (identity_file ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,99}$' AND identity_file !~ '\\.pub$');
