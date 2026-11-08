-- Register Guru/Tutor AWAL CHAERUDDIN, S.Pd. & Mutate KRISTIANTI
-- SK Pengangkatan Guru Tetap: 400.3/KPTS.033/Y-CN/VII/2025
-- SK Pembagian Tugas Mengajar: 400.3/41/ST-PKBM.ASF/VII/2025 (Pendidikan Pancasila)

DO $$
DECLARE
    v_tenant_id UUID := 'b158ec8a-6ea3-468a-bed5-917e37499550';
    v_guru_role_id UUID := 'eb158186-9685-4ae1-91eb-0bf62c8676e2';
    v_awal_user_id UUID := '4d3bf49e-2fc2-4ee4-be2d-337d0df3a093';
    v_awal_teacher_id UUID := 'e8680ef3-3adf-4d57-a6fd-9fc918aef1da';
    v_kristianti_teacher_id UUID := '01a096e3-cbf5-7041-a07c-720d92034dfa';
    v_kristianti_user_id UUID := '01a096e3-cbf1-7923-acc8-1b03dbe6e0e1';
BEGIN
    -- 1. Ensure User Account for Pak Awal Chaeruddin
    INSERT INTO users (
        id, tenant_id, email, username, password_hash, full_name, is_active, created_at, updated_at
    ) VALUES (
        v_awal_user_id,
        v_tenant_id,
        'awal.chaeruddin@guru.schoolos.id',
        'awal.chaeruddin',
        '$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY',
        'AWAL CHAERUDDIN, S.Pd.',
        true,
        NOW(),
        NOW()
    ) ON CONFLICT (id) DO UPDATE SET
        full_name = 'AWAL CHAERUDDIN, S.Pd.',
        username = 'awal.chaeruddin',
        is_active = true,
        updated_at = NOW();

    -- 2. Ensure Teacher Profile for Pak Awal Chaeruddin
    INSERT INTO teachers (
        id, tenant_id, user_id, nip, full_name, subject, nuptk, jk,
        tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk,
        agama, alamat_jalan, no_hp, email, is_active, created_at, updated_at
    ) VALUES (
        v_awal_teacher_id,
        v_tenant_id,
        v_awal_user_id,
        '3209102501990003',
        'AWAL CHAERUDDIN, S.Pd.',
        'Pendidikan Pancasila',
        '-',
        'L',
        'Cirebon',
        '1999-01-25',
        'Guru Tetap PKBM',
        'Guru',
        'Islam',
        'Dusun Pon 1 RT 001/004, Desa Mertapada Wetan, Kec. Astanajapura, Kab. Cirebon',
        '-',
        'awal.chaeruddin@guru.schoolos.id',
        true,
        NOW(),
        NOW()
    ) ON CONFLICT (id) DO UPDATE SET
        user_id = v_awal_user_id,
        nip = '3209102501990003',
        full_name = 'AWAL CHAERUDDIN, S.Pd.',
        subject = 'Pendidikan Pancasila',
        status_kepegawaian = 'Guru Tetap PKBM',
        is_active = true,
        deleted_at = NULL,
        updated_at = NOW();

    -- 3. Ensure Role Assignment
    INSERT INTO user_roles (user_id, role_id)
    VALUES (v_awal_user_id, v_guru_role_id)
    ON CONFLICT (user_id, role_id) DO NOTHING;

    -- 4. Reassign Schedules, Sessions, Materials, and Assignments
    UPDATE class_schedules
    SET teacher_id = v_awal_teacher_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_kristianti_teacher_id;

    UPDATE learning_sessions
    SET teacher_id = v_awal_teacher_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_kristianti_teacher_id;

    UPDATE learning_materials
    SET teacher_id = v_awal_teacher_id, created_by = v_awal_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_kristianti_teacher_id;

    UPDATE assignments
    SET teacher_id = v_awal_teacher_id, created_by = v_awal_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_kristianti_teacher_id;

    UPDATE quizzes
    SET teacher_id = v_awal_teacher_id, created_by = v_awal_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_kristianti_teacher_id;

    UPDATE inquiry_threads
    SET teacher_id = v_awal_teacher_id, teacher_name = 'AWAL CHAERUDDIN, S.Pd.', updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND (teacher_id = v_kristianti_teacher_id OR teacher_id = v_kristianti_user_id OR teacher_name ILIKE '%kristi%');

    -- 5. Mutate Kristianti
    UPDATE teachers
    SET is_active = false, status_kepegawaian = 'Mutasi Keluar', deleted_at = NOW(), updated_at = NOW()
    WHERE id = v_kristianti_teacher_id;

    UPDATE users
    SET is_active = false, updated_at = NOW()
    WHERE id = v_kristianti_user_id;

    UPDATE user_qr_tokens
    SET is_active = false, updated_at = NOW()
    WHERE user_id = v_kristianti_user_id;
END $$;

