-- Sync multi-subject teacher TAUFIQ HIDAYAT and other teachers with multiple subjects from schedules
UPDATE teachers t
SET
    subject = s_agg.combined_subjects,
    updated_at = NOW()
FROM (
    SELECT cs.teacher_id, STRING_AGG(
        DISTINCT s.name, ', '
        ORDER BY s.name
    ) as combined_subjects
    FROM class_schedules cs
    JOIN subjects s ON s.id = cs.subject_id
    WHERE cs.deleted_at IS NULL
      AND s.is_active = true
    GROUP BY cs.teacher_id
) s_agg
WHERE t.id = s_agg.teacher_id
  AND (t.subject IS NULL OR t.subject != s_agg.combined_subjects);
