-- 20260917070000_make_library_books_global.sql
-- Enable Global National Library Catalog (Kemendikdasmen Kurikulum Merdeka)
-- 
-- DATABASE INVARIANTS:
-- 1. library_books:
--    - tenant_id IS NULL     => National/Global Standard Catalog (Public across all tenants)
--    - tenant_id IS NOT NULL => School-specific Local Catalog
-- 2. learning_materials & reading_progress (Reading Assignments):
--    - tenant_id IS NOT NULL => Strictly isolated per tenant school
--    - class_id IS NOT NULL  => Strictly isolated to enrolled students of that class
--    - teacher_id/created_by => Strictly owned by assigning teacher (no cross-teacher leaks)

-- 1. Make tenant_id nullable so national books can exist without a tenant
ALTER TABLE library_books ALTER COLUMN tenant_id DROP NOT NULL;

-- 2. Add class_level, subject_name, and canonical_subject for universal matching
ALTER TABLE library_books ADD COLUMN IF NOT EXISTS class_level INTEGER;
ALTER TABLE library_books ADD COLUMN IF NOT EXISTS subject_name VARCHAR(100);
ALTER TABLE library_books ADD COLUMN IF NOT EXISTS canonical_subject VARCHAR(50);

-- 3. Indexes for fast lookup
CREATE INDEX IF NOT EXISTS idx_library_books_class_level ON library_books(class_level);
CREATE INDEX IF NOT EXISTS idx_library_books_subject_name ON library_books(subject_name);
CREATE INDEX IF NOT EXISTS idx_library_books_canonical_subject ON library_books(canonical_subject);
DROP INDEX IF EXISTS idx_library_books_global_file_url;
CREATE UNIQUE INDEX IF NOT EXISTS idx_library_books_global_title_file ON library_books (title, file_url) WHERE tenant_id IS NULL;
