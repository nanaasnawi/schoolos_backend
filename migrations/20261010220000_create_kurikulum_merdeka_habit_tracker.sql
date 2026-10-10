-- 20261010220000_create_kurikulum_merdeka_habit_tracker.sql
-- Fase 5: Gerakan 7 Kebiasaan Anak Indonesia Hebat (G7KAIH) & Verifikasi Wali
-- Bounded Context: Longitudinal Character & Daily Habit Tracking (BSKAP / Kemendikdasmen RI)

CREATE TABLE IF NOT EXISTS habit_tracker_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    entry_date DATE NOT NULL,
    academic_year VARCHAR(20) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL DEFAULT 'GANJIL' CHECK (semester IN ('ODD', 'EVEN', 'GANJIL', 'GENAP')),
    
    -- Canonical 7 Habits (JSONB for optimal storage & schema elasticity)
    -- Keys: bangun_pagi, beribadah, berolahraga, makan_sehat, gemar_belajar, bermasyarakat, tidur_tepat_waktu
    habits JSONB NOT NULL DEFAULT '{
        "bangun_pagi": false,
        "beribadah": false,
        "berolahraga": false,
        "makan_sehat": false,
        "gemar_belajar": false,
        "bermasyarakat": false,
        "tidur_tepat_waktu": false
    }'::jsonb,
    
    -- Optional qualitative notes/details per habit (e.g. {"makan_sehat": "Sayur bayam & buah pisang", "gemar_belajar": "Membaca buku sains 25 menit"})
    habits_notes JSONB NOT NULL DEFAULT '{}'::jsonb,
    
    -- Fast-lookup precomputed metrics
    completed_count SMALLINT NOT NULL DEFAULT 0 CHECK (completed_count BETWEEN 0 AND 7),
    compliance_rate NUMERIC(5,2) NOT NULL DEFAULT 0.00 CHECK (compliance_rate BETWEEN 0.00 AND 100.00),
    
    -- Verification & Fallback Governance
    -- PENDING: Siswa sudah mengisi, menunggu verifikasi orang tua
    -- PARENT_VERIFIED: Diverifikasi & disetujui orang tua/wali murid
    -- TEACHER_OVERRIDE: Diverifikasi oleh Wali Kelas (Fallback karena ortu pasif / batas 7 hari terlewati)
    -- SYSTEM_EXPIRED: Melebihi fallback window (7 hari) tanpa verifikasi ortu, siap di-override wali kelas
    verification_status VARCHAR(25) NOT NULL DEFAULT 'PENDING' 
        CHECK (verification_status IN ('PENDING', 'PARENT_VERIFIED', 'TEACHER_OVERRIDE', 'SYSTEM_EXPIRED')),
        
    verified_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    verified_by_role VARCHAR(25) CHECK (verified_by_role IN ('PARENT', 'GUARDIAN', 'HOMEROOM_TEACHER', 'SYSTEM')),
    verified_at TIMESTAMPTZ,
    
    parent_feedback TEXT,
    teacher_notes TEXT,
    override_reason TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT uq_habit_entry_student_date UNIQUE (tenant_id, student_id, entry_date)
);

-- Performance Indexes
CREATE INDEX IF NOT EXISTS idx_habit_entries_student_date 
    ON habit_tracker_entries (tenant_id, student_id, entry_date DESC);

CREATE INDEX IF NOT EXISTS idx_habit_entries_semester 
    ON habit_tracker_entries (tenant_id, student_id, academic_year, semester);

CREATE INDEX IF NOT EXISTS idx_habit_entries_verification 
    ON habit_tracker_entries (tenant_id, verification_status, entry_date);

CREATE INDEX IF NOT EXISTS idx_habit_entries_habits_gin 
    ON habit_tracker_entries USING gin (habits);

-- Trigger Function: Auto calculate completed_count and compliance_rate on insert/update
CREATE OR REPLACE FUNCTION fn_calculate_habit_metrics()
RETURNS TRIGGER AS $$
DECLARE
    v_count INT := 0;
BEGIN
    -- Check each canonical habit
    IF (NEW.habits->>'bangun_pagi')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'beribadah')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'berolahraga')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'makan_sehat')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'gemar_belajar')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'bermasyarakat')::boolean IS TRUE THEN v_count := v_count + 1; END IF;
    IF (NEW.habits->>'tidur_tepat_waktu')::boolean IS TRUE THEN v_count := v_count + 1; END IF;

    NEW.completed_count := v_count;
    NEW.compliance_rate := ROUND((v_count::numeric / 7.0) * 100.0, 2);
    NEW.updated_at := NOW();

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_calculate_habit_metrics ON habit_tracker_entries;
CREATE TRIGGER trg_calculate_habit_metrics
    BEFORE INSERT OR UPDATE OF habits ON habit_tracker_entries
    FOR EACH ROW
    EXECUTE FUNCTION fn_calculate_habit_metrics();

-- Stored Procedure / Function to mark pending habits as SYSTEM_EXPIRED if > 7 days old
CREATE OR REPLACE FUNCTION fn_expire_stale_habit_entries(
    p_tenant_id UUID,
    p_days_threshold INT DEFAULT 7
)
RETURNS INT AS $$
DECLARE
    v_updated_rows INT;
BEGIN
    UPDATE habit_tracker_entries
    SET verification_status = 'SYSTEM_EXPIRED',
        updated_at = NOW()
    WHERE tenant_id = p_tenant_id
      AND verification_status = 'PENDING'
      AND entry_date < (CURRENT_DATE - (p_days_threshold || ' days')::interval)::date;

    GET DIAGNOSTICS v_updated_rows = ROW_COUNT;
    RETURN v_updated_rows;
END;
$$ LANGUAGE plpgsql;
