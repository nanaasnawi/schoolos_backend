-- Deduplicate HUSNUL CHATIM vs HUSNUL CHATIM, S.Pd in teachers & inquiry_threads
-- Keeps the canonical teacher record (linked to user_id / Dapodik), reassigns all references,
-- removes the unused duplicate row, and standardizes the display name to HUSNUL CHATIM.

DO $$
DECLARE
    r RECORD;
    keep_id UUID;
    keep_user_id UUID;
BEGIN
    FOR r IN (
        SELECT tenant_id
        FROM teachers
        WHERE deleted_at IS NULL AND full_name ILIKE '%HUSNUL CHATIM%'
        GROUP BY tenant_id
    ) LOOP
        SELECT id, user_id INTO keep_id, keep_user_id
        FROM teachers
        WHERE tenant_id = r.tenant_id
          AND deleted_at IS NULL
          AND full_name ILIKE '%HUSNUL CHATIM%'
        ORDER BY (user_id IS NOT NULL) DESC, (nuptk IS NOT NULL) DESC, updated_at DESC, created_at ASC
        LIMIT 1;

        IF keep_id IS NOT NULL THEN
            UPDATE classes
            SET homeroom_teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND homeroom_teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE class_schedules
            SET teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE learning_sessions
            SET teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE learning_materials
            SET teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE assignments
            SET teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE quizzes
            SET teacher_id = keep_id, updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND teacher_id IN (
                  SELECT id FROM teachers
                  WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%' AND id != keep_id
              );

            UPDATE inquiry_threads
            SET teacher_id = COALESCE(keep_user_id, keep_id),
                teacher_name = 'HUSNUL CHATIM',
                updated_at = NOW()
            WHERE tenant_id = r.tenant_id
              AND (
                  teacher_name ILIKE '%HUSNUL CHATIM%'
                  OR teacher_id IN (
                      SELECT id FROM teachers
                      WHERE tenant_id = r.tenant_id AND full_name ILIKE '%HUSNUL CHATIM%'
                  )
              );

            DELETE FROM teachers
            WHERE tenant_id = r.tenant_id
              AND full_name ILIKE '%HUSNUL CHATIM%'
              AND id != keep_id;

            UPDATE teachers
            SET full_name = 'HUSNUL CHATIM',
                subject = COALESCE(NULLIF(TRIM(subject), ''), 'Seni Budaya'),
                updated_at = NOW()
            WHERE id = keep_id;

            IF keep_user_id IS NOT NULL THEN
                UPDATE users
                SET full_name = 'HUSNUL CHATIM',
                    updated_at = NOW()
                WHERE id = keep_user_id AND full_name ILIKE '%HUSNUL CHATIM%';
            END IF;
        END IF;
    END LOOP;
END $$;

