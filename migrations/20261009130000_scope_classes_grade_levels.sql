-- 20261009130000_scope_classes_grade_levels.sql
-- Ensure grade levels 1-12 exist for all tenants and link classes properly for SIBI book matching

DO $$
DECLARE
    t_id UUID;
    lvl INT;
BEGIN
    FOR t_id IN SELECT id FROM tenants LOOP
        FOR lvl IN 1..12 LOOP
            INSERT INTO grade_levels (id, tenant_id, name, level, created_at, updated_at)
            VALUES (gen_random_uuid(), t_id, 'Kelas ' || lvl, lvl, NOW(), NOW())
            ON CONFLICT DO NOTHING;
        END LOOP;
    END LOOP;
END $$;

-- Update classes tingkat and grade_level_id based on number in class name
UPDATE classes c
SET tingkat = substring(c.name from '[0-9]+'),
    grade_level_id = gl.id,
    updated_at = NOW()
FROM grade_levels gl
WHERE gl.tenant_id = c.tenant_id
  AND gl.level = substring(c.name from '[0-9]+')::int
  AND substring(c.name from '[0-9]+') IS NOT NULL;
