ALTER TABLE plugin_memory_sources DROP CONSTRAINT plugin_memory_sources_status_check;
ALTER TABLE plugin_memory_sources ADD CONSTRAINT plugin_memory_sources_status_check
    CHECK (status IN ('pending','quarantined','processing','complete','conflict','failed','deleted','recorded'));
