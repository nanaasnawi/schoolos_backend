-- 20261108233000_register_nana_asnawi_teacher_and_mutate_ikin.sql
-- Register NANA ASNAWI as Teacher/Tutor for Keterampilan Komputer & Mutate IKIN BAIHAKI
-- SK Pembagian Tugas: Operator & Guru Pengampu Keterampilan Komputer PKBM As-Salafiyah

DO $$
DECLARE
    v_tenant_id UUID := 'b158ec8a-6ea3-468a-bed5-917e37499550';
    v_guru_role_id UUID := 'eb158186-9685-4ae1-91eb-0bf62c8676e2';
    v_nana_user_id UUID := '01a1593b-a8cc-7cc3-bd5f-2a7f49c15948';
    v_nana_teacher_id UUID := '01a1593b-a8cc-7cc3-bd5f-2a7f49c15949';
    v_ikin_teacher_id UUID := '46ce30ba-b618-42d9-8d6d-31d698331353';
    v_ikin_user_id UUID := '01a096e3-cbd9-78d3-8a4d-54e97cb12b92';
BEGIN
    -- 1. Ensure User Account for Nana Asnawi
    UPDATE users
    SET username = COALESCE(username, 'nana.asnawi'),
        full_name = 'NANA ASNAWI',
        is_active = true,
        updated_at = NOW()
    WHERE id = v_nana_user_id;

    -- 2. Insert or update Teacher Profile for Nana Asnawi
    INSERT INTO teachers (
        id, tenant_id, user_id, nip, full_name, subject, nuptk, jk,
        status_kepegawaian, jenis_ptk, email, foto_profile, is_active,
        created_at, updated_at
    ) VALUES (
        v_nana_teacher_id,
        v_tenant_id,
        v_nana_user_id,
        NULL,
        'NANA ASNAWI',
        'Keterampilan Komputer',
        NULL,
        'L',
        'Tenaga Kependidikan & Guru',
        'Guru / Tenaga Administrasi',
        'nanaadrianzyah7@gmail.com',
        '/api/v1/auth/avatar/01a1593b-a8cc-7cc3-bd5f-2a7f49c15948',
        true,
        NOW(),
        NOW()
    ) ON CONFLICT (id) DO UPDATE SET
        user_id = v_nana_user_id,
        full_name = 'NANA ASNAWI',
        subject = 'Keterampilan Komputer',
        status_kepegawaian = 'Tenaga Kependidikan & Guru',
        jenis_ptk = 'Guru / Tenaga Administrasi',
        email = 'nanaadrianzyah7@gmail.com',
        foto_profile = '/api/v1/auth/avatar/01a1593b-a8cc-7cc3-bd5f-2a7f49c15948',
        is_active = true,
        deleted_at = NULL,
        updated_at = NOW();

    -- 3. Ensure Dual Role Assignment (Guru role alongside existing Operator/Staff role)
    INSERT INTO user_roles (user_id, role_id)
    VALUES (v_nana_user_id, v_guru_role_id)
    ON CONFLICT (user_id, role_id) DO NOTHING;

    -- 4. Reassign Schedules, Sessions, Materials, Quizzes, and Assignments
    UPDATE class_schedules
    SET teacher_id = v_nana_teacher_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_ikin_teacher_id;

    UPDATE learning_sessions
    SET teacher_id = v_nana_teacher_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_ikin_teacher_id;

    UPDATE assignments
    SET teacher_id = v_nana_teacher_id, created_by = v_nana_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_ikin_teacher_id;

    UPDATE learning_materials
    SET teacher_id = v_nana_teacher_id, created_by = v_nana_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_ikin_teacher_id;

    UPDATE quizzes
    SET teacher_id = v_nana_teacher_id, created_by = v_nana_user_id, updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND teacher_id = v_ikin_teacher_id;

    UPDATE inquiry_threads
    SET teacher_id = v_nana_teacher_id, teacher_name = 'NANA ASNAWI', updated_at = NOW()
    WHERE tenant_id = v_tenant_id AND (teacher_id = v_ikin_teacher_id OR teacher_id = v_ikin_user_id OR teacher_name ILIKE '%ikin%');

    -- 5. Mutate Ikin Baihaki
    UPDATE teachers
    SET is_active = false, status_kepegawaian = 'Mutasi Keluar', deleted_at = NOW(), updated_at = NOW()
    WHERE id = v_ikin_teacher_id;

    UPDATE users
    SET is_active = false, updated_at = NOW()
    WHERE id = v_ikin_user_id;

    UPDATE user_qr_tokens
    SET is_active = false, updated_at = NOW()
    WHERE user_id = v_ikin_user_id;
END $$;

