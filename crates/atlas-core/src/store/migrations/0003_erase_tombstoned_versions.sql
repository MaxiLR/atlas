-- A section version tombstoned earlier can still have its content erased,
-- as deleting its course requires: the one other update allowed keeps its
-- tombstone and empties its blocks, checks, citations, sources and record.
DROP TRIGGER section_versions_are_immutable;

CREATE TRIGGER section_versions_are_immutable
BEFORE UPDATE ON section_versions
WHEN NOT (NEW.id = OLD.id AND NEW.rowid = OLD.rowid AND NEW.node_id = OLD.node_id
          AND NEW.created_at = OLD.created_at
          AND ((OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL)
               OR (OLD.deleted_at IS NOT NULL AND NEW.deleted_at = OLD.deleted_at
                   AND NEW.blocks = '' AND NEW.checks = '' AND NEW.citations = ''
                   AND NEW.sources = '' AND NEW.record IS NULL)))
BEGIN
    SELECT RAISE(ABORT, 'a section version is immutable; only its tombstone can be set');
END;
