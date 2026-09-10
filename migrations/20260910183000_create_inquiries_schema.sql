-- Create inquiry_threads table
CREATE TABLE IF NOT EXISTS inquiry_threads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    student_name VARCHAR(255) NOT NULL,
    student_class VARCHAR(100) NOT NULL DEFAULT '',
    teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL,
    teacher_name VARCHAR(255) NOT NULL DEFAULT 'Guru Pengampu',
    subject_name VARCHAR(100) NOT NULL DEFAULT 'Umum',
    inquiry_type VARCHAR(50) NOT NULL DEFAULT 'MATERIAL',
    reference_title VARCHAR(255) NOT NULL,
    reference_id VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'WAITING_REPLY',
    last_message_content TEXT,
    last_message_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inquiry_threads_tenant ON inquiry_threads(tenant_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_threads_student ON inquiry_threads(student_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_threads_teacher ON inquiry_threads(teacher_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_threads_status ON inquiry_threads(status);
CREATE INDEX IF NOT EXISTS idx_inquiry_threads_last_msg ON inquiry_threads(last_message_at DESC);

-- Create inquiry_messages table
CREATE TABLE IF NOT EXISTS inquiry_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    thread_id UUID NOT NULL REFERENCES inquiry_threads(id) ON DELETE CASCADE,
    sender_id VARCHAR(255) NOT NULL,
    sender_name VARCHAR(255) NOT NULL,
    sender_role VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    is_from_teacher BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inquiry_messages_thread ON inquiry_messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_messages_tenant ON inquiry_messages(tenant_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_messages_created ON inquiry_messages(created_at ASC);

-- Initial seed connecting real database students & materials
DO $$
DECLARE
    v_tenant_id UUID;
    v_std1 RECORD;
    v_std2 RECORD;
    v_std3 RECORD;
    v_mat1 RECORD;
    v_mat2 RECORD;
    v_asg1 RECORD;
    v_t1_id UUID;
    v_t2_id UUID;
    v_t3_id UUID;
BEGIN
    SELECT id INTO v_tenant_id FROM tenants LIMIT 1;
    IF v_tenant_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM inquiry_threads LIMIT 1) THEN
        SELECT s.id, s.full_name, COALESCE(c.name, 'PAKET B7') as class_name 
        INTO v_std1 
        FROM students s 
        LEFT JOIN enrollments en ON en.student_id = s.id 
        LEFT JOIN classes c ON c.id = en.class_id 
        ORDER BY s.created_at ASC LIMIT 1;

        SELECT s.id, s.full_name, COALESCE(c.name, 'PAKET C11a') as class_name 
        INTO v_std2 
        FROM students s 
        LEFT JOIN enrollments en ON en.student_id = s.id 
        LEFT JOIN classes c ON c.id = en.class_id 
        ORDER BY s.created_at ASC OFFSET 1 LIMIT 1;

        SELECT s.id, s.full_name, COALESCE(c.name, 'PAKET C12a') as class_name 
        INTO v_std3 
        FROM students s 
        LEFT JOIN enrollments en ON en.student_id = s.id 
        LEFT JOIN classes c ON c.id = en.class_id 
        ORDER BY s.created_at ASC OFFSET 2 LIMIT 1;

        SELECT id, title INTO v_mat1 FROM learning_materials ORDER BY created_at ASC LIMIT 1;
        SELECT id, title INTO v_mat2 FROM learning_materials ORDER BY created_at ASC OFFSET 1 LIMIT 1;
        SELECT id, title INTO v_asg1 FROM assignments ORDER BY created_at ASC LIMIT 1;

        IF v_std1.id IS NOT NULL AND v_mat1.id IS NOT NULL THEN
            v_t1_id := gen_random_uuid();
            INSERT INTO inquiry_threads (id, tenant_id, student_id, student_name, student_class, teacher_name, subject_name, inquiry_type, reference_title, reference_id, status, last_message_content, last_message_at, created_at, updated_at)
            VALUES (v_t1_id, v_tenant_id, v_std1.id, v_std1.full_name, v_std1.class_name, 'HASSAN MUSTOFA', 'Ilmu Pengetahuan Alam', 'MATERIAL', v_mat1.title, v_mat1.id::text, 'WAITING_REPLY', 'Assalamu''alaikum pak, saya ingin bertanya tentang klasifikasi makhluk hidup pada slide ke-3, perbedaannya bagaimana ya pak?', NOW() - INTERVAL '15 minutes', NOW() - INTERVAL '15 minutes', NOW() - INTERVAL '15 minutes');

            INSERT INTO inquiry_messages (tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at)
            VALUES (v_tenant_id, v_t1_id, v_std1.id::text, v_std1.full_name, 'STUDENT', 'Assalamu''alaikum pak, saya ingin bertanya tentang klasifikasi makhluk hidup pada slide ke-3, perbedaannya bagaimana ya pak?', false, NOW() - INTERVAL '15 minutes');
        END IF;

        IF v_std2.id IS NOT NULL AND v_asg1.id IS NOT NULL THEN
            v_t2_id := gen_random_uuid();
            INSERT INTO inquiry_threads (id, tenant_id, student_id, student_name, student_class, teacher_name, subject_name, inquiry_type, reference_title, reference_id, status, last_message_content, last_message_at, created_at, updated_at)
            VALUES (v_t2_id, v_tenant_id, v_std2.id, v_std2.full_name, v_std2.class_name, 'HASSAN MUSTOFA', 'Fisika Terapan', 'ASSIGNMENT', v_asg1.title, v_asg1.id::text, 'ANSWERED', 'Wa''alaikumussalam. Untuk nomor 2 gunakan satuan baku SI ya agar hasilnya akurat.', NOW() - INTERVAL '40 minutes', NOW() - INTERVAL '2 hours', NOW() - INTERVAL '40 minutes');

            INSERT INTO inquiry_messages (tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at)
            VALUES 
            (v_tenant_id, v_t2_id, v_std2.id::text, v_std2.full_name, 'STUDENT', 'Pak guru, untuk tugas analisis pengukuran nomor 2 apakah boleh satuannya cm atau harus meter (SI)?', false, NOW() - INTERVAL '2 hours'),
            (v_tenant_id, v_t2_id, 'teacher-hassan', 'HASSAN MUSTOFA', 'TEACHER', 'Wa''alaikumussalam. Untuk nomor 2 gunakan satuan baku SI ya agar hasilnya akurat.', true, NOW() - INTERVAL '40 minutes');
        END IF;

        IF v_std3.id IS NOT NULL AND v_mat2.id IS NOT NULL THEN
            v_t3_id := gen_random_uuid();
            INSERT INTO inquiry_threads (id, tenant_id, student_id, student_name, student_class, teacher_name, subject_name, inquiry_type, reference_title, reference_id, status, last_message_content, last_message_at, created_at, updated_at)
            VALUES (v_t3_id, v_tenant_id, v_std3.id, v_std3.full_name, v_std3.class_name, 'IKIN BAIHAKI', 'Biologi Terapan', 'MATERIAL', v_mat2.title, v_mat2.id::text, 'WAITING_REPLY', 'Pak Ikin, apakah rantai makanan dekomposer bisa mempengaruhi populasi produsen secara langsung?', NOW() - INTERVAL '3 hours', NOW() - INTERVAL '3 hours', NOW() - INTERVAL '3 hours');

            INSERT INTO inquiry_messages (tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at)
            VALUES (v_tenant_id, v_t3_id, v_std3.id::text, v_std3.full_name, 'STUDENT', 'Pak Ikin, apakah rantai makanan dekomposer bisa mempengaruhi populasi produsen secara langsung?', false, NOW() - INTERVAL '3 hours');
        END IF;
    END IF;
END $$;
