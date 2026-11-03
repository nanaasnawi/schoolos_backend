-- Sync multi-subject teachers from class_schedules into teachers.subject
-- Ensures teachers who teach multiple distinct subjects (e.g. TAUFIQ HIDAYAT teaching Geografi & IPS)
-- have all their taught subjects listed as comma-separated values in teachers.subject

UPDATE teachers t
SET
    subject = s_agg.combined_subjects,
    updated_at = NOW()
FROM (
        SELECT cs.teacher_id, STRING_AGG(
                DISTINCT s.name, ', '
                ORDER BY s.name
            ) as combined_subjects
        FROM
            class_schedules cs
            JOIN subjects s ON s.id = cs.subject_id
        WHERE
            cs.deleted_at IS NULL
            AND s.is_active = true
        GROUP BY
            cs.teacher_id
    ) s_agg
WHERE
    t.id = s_agg.teacher_id
    AND (
        t.subject IS NULL
        OR t.subject != s_agg.combined_subjects
    );