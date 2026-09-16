-- 20260917050000_create_learning_library.sql
-- Teacher Learning Library as optional shortcut, reading progress tracking & analytics

-- 1. Create library_books table
CREATE TABLE IF NOT EXISTS library_books (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    author VARCHAR(255),
    publisher VARCHAR(255),
    subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
    total_pages INTEGER NOT NULL DEFAULT 100,
    cover_url TEXT,
    file_url TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_library_books_tenant ON library_books(tenant_id);
CREATE INDEX IF NOT EXISTS idx_library_books_subject ON library_books(subject_id);

-- 2. Extend learning_materials with optional library source metadata
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS source_type VARCHAR(20) NOT NULL DEFAULT 'MANUAL'; -- 'MANUAL' or 'LIBRARY'
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS library_book_id UUID REFERENCES library_books(id) ON DELETE SET NULL;
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS start_page INTEGER;
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS end_page INTEGER;

CREATE INDEX IF NOT EXISTS idx_learning_materials_library_book ON learning_materials(library_book_id);

-- 3. Create reading_progress table for tracking student reading assignments
CREATE TABLE IF NOT EXISTS reading_progress (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    material_id UUID NOT NULL REFERENCES learning_materials(id) ON DELETE CASCADE,
    current_page INTEGER NOT NULL DEFAULT 1,
    is_completed BOOLEAN NOT NULL DEFAULT false,
    last_read_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (student_id, material_id)
);

CREATE INDEX IF NOT EXISTS idx_reading_progress_student ON reading_progress(student_id);
CREATE INDEX IF NOT EXISTS idx_reading_progress_material ON reading_progress(material_id);
