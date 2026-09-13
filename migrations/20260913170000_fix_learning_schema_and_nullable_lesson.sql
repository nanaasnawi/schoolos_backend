-- Migration: 20260913170000_fix_learning_schema_and_nullable_lesson.sql
-- Description: Make lesson_id nullable on assignments and quizzes, and add missing relation columns (class_id, subject_id, teacher_id, created_by)

-- 1. Make lesson_id nullable and safe on assignments
ALTER TABLE assignments ALTER COLUMN lesson_id DROP NOT NULL;
ALTER TABLE assignments DROP CONSTRAINT IF EXISTS assignments_lesson_id_fkey;
ALTER TABLE assignments ADD CONSTRAINT assignments_lesson_id_fkey 
    FOREIGN KEY (lesson_id) REFERENCES lessons(id) ON DELETE SET NULL;

-- 2. Make lesson_id nullable and safe on quizzes
ALTER TABLE quizzes ALTER COLUMN lesson_id DROP NOT NULL;
ALTER TABLE quizzes DROP CONSTRAINT IF EXISTS quizzes_lesson_id_fkey;
ALTER TABLE quizzes ADD CONSTRAINT quizzes_lesson_id_fkey 
    FOREIGN KEY (lesson_id) REFERENCES lessons(id) ON DELETE SET NULL;

-- 3. Add missing relation columns to learning_materials
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS class_id UUID REFERENCES classes(id) ON DELETE SET NULL;
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL;
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL;
ALTER TABLE learning_materials ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_learning_materials_class_id ON learning_materials(class_id);
CREATE INDEX IF NOT EXISTS idx_learning_materials_subject_id ON learning_materials(subject_id);
CREATE INDEX IF NOT EXISTS idx_learning_materials_teacher_id ON learning_materials(teacher_id);
CREATE INDEX IF NOT EXISTS idx_learning_materials_created_by ON learning_materials(created_by);

-- 4. Add missing relation columns to assignments
ALTER TABLE assignments ADD COLUMN IF NOT EXISTS class_id UUID REFERENCES classes(id) ON DELETE SET NULL;
ALTER TABLE assignments ADD COLUMN IF NOT EXISTS subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL;
ALTER TABLE assignments ADD COLUMN IF NOT EXISTS teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL;
ALTER TABLE assignments ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_assignments_class_id ON assignments(class_id);
CREATE INDEX IF NOT EXISTS idx_assignments_subject_id ON assignments(subject_id);
CREATE INDEX IF NOT EXISTS idx_assignments_teacher_id ON assignments(teacher_id);
CREATE INDEX IF NOT EXISTS idx_assignments_created_by ON assignments(created_by);

-- 5. Add missing relation columns to quizzes
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS class_id UUID REFERENCES classes(id) ON DELETE SET NULL;
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL;
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL;
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_quizzes_class_id ON quizzes(class_id);
CREATE INDEX IF NOT EXISTS idx_quizzes_subject_id ON quizzes(subject_id);
CREATE INDEX IF NOT EXISTS idx_quizzes_teacher_id ON quizzes(teacher_id);
CREATE INDEX IF NOT EXISTS idx_quizzes_created_by ON quizzes(created_by);
