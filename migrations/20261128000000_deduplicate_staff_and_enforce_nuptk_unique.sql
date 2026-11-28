-- 20261128000000_deduplicate_staff_and_enforce_nuptk_unique.sql
-- Merge duplicate active staff records sharing the same (tenant_id, TRIM(nuptk)) or (tenant_id, user_id),
-- keeping the canonical titled / linked row (e.g., "Siti Muniroh, S.Pd" over duplicate "SITI MUNIROH"),
-- soft-delete redundant duplicates, and prevent future duplicates via partial unique indexes.

-- 1. Deduplicate active staff sharing the same non-empty TRIM(nuptk) within a tenant
WITH ranked_nuptk AS (
    SELECT
        id,
        tenant_id,
        TRIM(nuptk) AS norm_nuptk,
        ROW_NUMBER() OVER (
            PARTITION BY tenant_id, TRIM(nuptk)
            ORDER BY
                CASE WHEN user_id IS NOT NULL THEN 0 ELSE 1 END,
                CASE WHEN POSITION(',' IN full_name) > 0 THEN 0 ELSE 1 END,
                CASE WHEN full_name <> UPPER(full_name) THEN 0 ELSE 1 END,
                LENGTH(full_name) DESC,
                created_at ASC,
                id ASC
        ) AS rn
    FROM staff
    WHERE deleted_at IS NULL
      AND nuptk IS NOT NULL
      AND TRIM(nuptk) <> ''
      AND TRIM(nuptk) <> '-'
),
dup_nuptk_groups AS (
    SELECT tenant_id, norm_nuptk
    FROM ranked_nuptk
    GROUP BY tenant_id, norm_nuptk
    HAVING COUNT(*) > 1
),
merged_nuptk AS (
    SELECT
        (ARRAY_AGG(s.id ORDER BY r.rn ASC))[1] AS keep_id,
        r.norm_nuptk,
        (ARRAY_AGG(s.user_id ORDER BY r.rn ASC) FILTER (WHERE s.user_id IS NOT NULL))[1] AS best_user_id,
        (ARRAY_AGG(s.full_name ORDER BY
            CASE WHEN POSITION(',' IN s.full_name) > 0 THEN 0 ELSE 1 END,
            CASE WHEN s.full_name <> UPPER(s.full_name) THEN 0 ELSE 1 END,
            LENGTH(s.full_name) DESC,
            r.rn ASC
        ))[1] AS best_full_name,
        (ARRAY_AGG(s.job_title ORDER BY
            CASE WHEN s.job_title = 'Kepala Sekolah' THEN 0
                 WHEN NULLIF(NULLIF(TRIM(s.job_title), ''), '-') IS NOT NULL AND s.job_title <> 'Tendik' THEN 1
                 ELSE 2 END,
            r.rn ASC
        ) FILTER (WHERE NULLIF(NULLIF(TRIM(s.job_title), ''), '-') IS NOT NULL))[1] AS best_job_title,
        (ARRAY_AGG(s.jk ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.jk), ''), '-') IS NOT NULL))[1] AS best_jk,
        (ARRAY_AGG(s.tempat_lahir ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.tempat_lahir), ''), '-') IS NOT NULL))[1] AS best_tempat_lahir,
        (ARRAY_AGG(s.tanggal_lahir ORDER BY r.rn ASC) FILTER (WHERE s.tanggal_lahir IS NOT NULL))[1] AS best_tanggal_lahir,
        (ARRAY_AGG(s.nip ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.nip), ''), '-') IS NOT NULL AND s.nip NOT LIKE 'TENDIK-%'))[1] AS best_real_nip,
        (ARRAY_AGG(s.nip ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.nip), ''), '-') IS NOT NULL))[1] AS best_any_nip,
        (ARRAY_AGG(s.status_kepegawaian ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.status_kepegawaian), ''), '-') IS NOT NULL))[1] AS best_status_kepegawaian,
        (ARRAY_AGG(s.jenis_ptk ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.jenis_ptk), ''), '-') IS NOT NULL))[1] AS best_jenis_ptk,
        (ARRAY_AGG(s.agama ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.agama), ''), '-') IS NOT NULL))[1] AS best_agama,
        (ARRAY_AGG(s.alamat_jalan ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.alamat_jalan), ''), '-') IS NOT NULL))[1] AS best_alamat_jalan,
        (ARRAY_AGG(s.no_hp ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.no_hp), ''), '-') IS NOT NULL))[1] AS best_no_hp,
        (ARRAY_AGG(s.email ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.email), ''), '-') IS NOT NULL))[1] AS best_email,
        BOOL_OR(s.is_active) AS any_active
    FROM ranked_nuptk r
    JOIN dup_nuptk_groups d ON d.tenant_id = r.tenant_id AND d.norm_nuptk = r.norm_nuptk
    JOIN staff s ON s.id = r.id
    GROUP BY r.tenant_id, r.norm_nuptk
),
upd_keep AS (
    UPDATE staff s
    SET
        user_id = COALESCE(s.user_id, m.best_user_id),
        full_name = COALESCE(m.best_full_name, s.full_name),
        nuptk = m.norm_nuptk,
        job_title = COALESCE(m.best_job_title, s.job_title),
        jk = COALESCE(NULLIF(NULLIF(TRIM(s.jk), ''), '-'), m.best_jk),
        tempat_lahir = COALESCE(NULLIF(NULLIF(TRIM(s.tempat_lahir), ''), '-'), m.best_tempat_lahir),
        tanggal_lahir = COALESCE(s.tanggal_lahir, m.best_tanggal_lahir),
        nip = COALESCE(m.best_real_nip, NULLIF(NULLIF(TRIM(s.nip), ''), '-'), m.best_any_nip),
        status_kepegawaian = COALESCE(NULLIF(NULLIF(TRIM(s.status_kepegawaian), ''), '-'), m.best_status_kepegawaian),
        jenis_ptk = COALESCE(NULLIF(NULLIF(TRIM(s.jenis_ptk), ''), '-'), m.best_jenis_ptk),
        agama = COALESCE(NULLIF(NULLIF(TRIM(s.agama), ''), '-'), m.best_agama),
        alamat_jalan = COALESCE(NULLIF(NULLIF(TRIM(s.alamat_jalan), ''), '-'), m.best_alamat_jalan),
        no_hp = COALESCE(NULLIF(NULLIF(TRIM(s.no_hp), ''), '-'), m.best_no_hp),
        email = COALESCE(NULLIF(NULLIF(TRIM(s.email), ''), '-'), m.best_email),
        is_active = m.any_active,
        updated_at = NOW()
    FROM merged_nuptk m
    WHERE s.id = m.keep_id
)
UPDATE staff s
SET
    deleted_at = NOW(),
    is_active = false,
    updated_at = NOW()
FROM ranked_nuptk r
JOIN dup_nuptk_groups d ON d.tenant_id = r.tenant_id AND d.norm_nuptk = r.norm_nuptk
WHERE s.id = r.id
  AND r.rn > 1
  AND s.deleted_at IS NULL;

-- 2. Deduplicate active staff sharing the same user_id within a tenant (in case nuptk was missing on one row)
WITH ranked_user AS (
    SELECT
        id,
        tenant_id,
        user_id,
        ROW_NUMBER() OVER (
            PARTITION BY tenant_id, user_id
            ORDER BY
                CASE WHEN NULLIF(TRIM(nuptk), '') IS NOT NULL THEN 0 ELSE 1 END,
                CASE WHEN POSITION(',' IN full_name) > 0 THEN 0 ELSE 1 END,
                CASE WHEN full_name <> UPPER(full_name) THEN 0 ELSE 1 END,
                LENGTH(full_name) DESC,
                created_at ASC,
                id ASC
        ) AS rn
    FROM staff
    WHERE deleted_at IS NULL
      AND user_id IS NOT NULL
),
dup_user_groups AS (
    SELECT tenant_id, user_id
    FROM ranked_user
    GROUP BY tenant_id, user_id
    HAVING COUNT(*) > 1
),
merged_user AS (
    SELECT
        (ARRAY_AGG(s.id ORDER BY r.rn ASC))[1] AS keep_id,
        (ARRAY_AGG(s.full_name ORDER BY
            CASE WHEN POSITION(',' IN s.full_name) > 0 THEN 0 ELSE 1 END,
            CASE WHEN s.full_name <> UPPER(s.full_name) THEN 0 ELSE 1 END,
            LENGTH(s.full_name) DESC,
            r.rn ASC
        ))[1] AS best_full_name,
        (ARRAY_AGG(TRIM(s.nuptk) ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.nuptk), ''), '-') IS NOT NULL))[1] AS best_nuptk,
        (ARRAY_AGG(s.job_title ORDER BY
            CASE WHEN s.job_title = 'Kepala Sekolah' THEN 0
                 WHEN NULLIF(NULLIF(TRIM(s.job_title), ''), '-') IS NOT NULL AND s.job_title <> 'Tendik' THEN 1
                 ELSE 2 END,
            r.rn ASC
        ) FILTER (WHERE NULLIF(NULLIF(TRIM(s.job_title), ''), '-') IS NOT NULL))[1] AS best_job_title,
        (ARRAY_AGG(s.jk ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.jk), ''), '-') IS NOT NULL))[1] AS best_jk,
        (ARRAY_AGG(s.tempat_lahir ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.tempat_lahir), ''), '-') IS NOT NULL))[1] AS best_tempat_lahir,
        (ARRAY_AGG(s.tanggal_lahir ORDER BY r.rn ASC) FILTER (WHERE s.tanggal_lahir IS NOT NULL))[1] AS best_tanggal_lahir,
        (ARRAY_AGG(s.nip ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.nip), ''), '-') IS NOT NULL))[1] AS best_nip,
        (ARRAY_AGG(s.status_kepegawaian ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.status_kepegawaian), ''), '-') IS NOT NULL))[1] AS best_status_kepegawaian,
        (ARRAY_AGG(s.jenis_ptk ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.jenis_ptk), ''), '-') IS NOT NULL))[1] AS best_jenis_ptk,
        (ARRAY_AGG(s.agama ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.agama), ''), '-') IS NOT NULL))[1] AS best_agama,
        (ARRAY_AGG(s.alamat_jalan ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.alamat_jalan), ''), '-') IS NOT NULL))[1] AS best_alamat_jalan,
        (ARRAY_AGG(s.no_hp ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.no_hp), ''), '-') IS NOT NULL))[1] AS best_no_hp,
        (ARRAY_AGG(s.email ORDER BY r.rn ASC) FILTER (WHERE NULLIF(NULLIF(TRIM(s.email), ''), '-') IS NOT NULL))[1] AS best_email,
        BOOL_OR(s.is_active) AS any_active
    FROM ranked_user r
    JOIN dup_user_groups d ON d.tenant_id = r.tenant_id AND d.user_id = r.user_id
    JOIN staff s ON s.id = r.id
    GROUP BY r.tenant_id, r.user_id
),
upd_keep_user AS (
    UPDATE staff s
    SET
        full_name = COALESCE(m.best_full_name, s.full_name),
        nuptk = COALESCE(NULLIF(NULLIF(TRIM(s.nuptk), ''), '-'), m.best_nuptk),
        job_title = COALESCE(m.best_job_title, s.job_title),
        jk = COALESCE(NULLIF(NULLIF(TRIM(s.jk), ''), '-'), m.best_jk),
        tempat_lahir = COALESCE(NULLIF(NULLIF(TRIM(s.tempat_lahir), ''), '-'), m.best_tempat_lahir),
        tanggal_lahir = COALESCE(s.tanggal_lahir, m.best_tanggal_lahir),
        nip = COALESCE(NULLIF(NULLIF(TRIM(s.nip), ''), '-'), m.best_nip),
        status_kepegawaian = COALESCE(NULLIF(NULLIF(TRIM(s.status_kepegawaian), ''), '-'), m.best_status_kepegawaian),
        jenis_ptk = COALESCE(NULLIF(NULLIF(TRIM(s.jenis_ptk), ''), '-'), m.best_jenis_ptk),
        agama = COALESCE(NULLIF(NULLIF(TRIM(s.agama), ''), '-'), m.best_agama),
        alamat_jalan = COALESCE(NULLIF(NULLIF(TRIM(s.alamat_jalan), ''), '-'), m.best_alamat_jalan),
        no_hp = COALESCE(NULLIF(NULLIF(TRIM(s.no_hp), ''), '-'), m.best_no_hp),
        email = COALESCE(NULLIF(NULLIF(TRIM(s.email), ''), '-'), m.best_email),
        is_active = m.any_active,
        updated_at = NOW()
    FROM merged_user m
    WHERE s.id = m.keep_id
)
UPDATE staff s
SET
    deleted_at = NOW(),
    is_active = false,
    updated_at = NOW()
FROM ranked_user r
JOIN dup_user_groups d ON d.tenant_id = r.tenant_id AND d.user_id = r.user_id
WHERE s.id = r.id
  AND r.rn > 1
  AND s.deleted_at IS NULL;

-- 3. Enforce uniqueness on active staff NUPTK and user_id per tenant
CREATE UNIQUE INDEX IF NOT EXISTS idx_staff_tenant_nuptk_unique
    ON staff (tenant_id, TRIM(nuptk))
    WHERE nuptk IS NOT NULL AND TRIM(nuptk) <> '' AND TRIM(nuptk) <> '-' AND deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_staff_tenant_user_id_unique
    ON staff (tenant_id, user_id)
    WHERE user_id IS NOT NULL AND deleted_at IS NULL;
