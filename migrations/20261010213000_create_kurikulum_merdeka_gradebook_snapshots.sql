-- 20261010213000_create_kurikulum_merdeka_gradebook_snapshots.sql
-- Kurikulum Merdeka: Materialized GradeBook Aggregation & Snapshot Cache
-- Mengatasi spike load saat cetak rapor massal dan menjaga konsistensi nilai transaksional

CREATE TABLE IF NOT EXISTS gradebook_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL CHECK (semester IN ('ODD', 'EVEN')),
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    subject_code VARCHAR(50) NOT NULL,
    subject_name VARCHAR(100) NOT NULL,
    
    -- Nilai Agregat Terkunci (Pre-Calculated)
    avg_summative_tp NUMERIC(5,2) NOT NULL,
    sas_score NUMERIC(5,2) DEFAULT NULL,
    final_score NUMERIC(5,2) NOT NULL,
    status_kktp VARCHAR(20) NOT NULL CHECK (status_kktp IN ('TUNTAS', 'PERLU_REMEDIAL')),
    
    -- Teks Narasi Rapor Siap Cetak (Instant Retrieval)
    highest_lo_id UUID REFERENCES learning_objectives(id) ON DELETE SET NULL,
    highest_achievement_desc TEXT NOT NULL,
    lowest_lo_id UUID REFERENCES learning_objectives(id) ON DELETE SET NULL,
    lowest_improvement_desc TEXT NOT NULL,
    
    -- Cache Lifecycle State
    is_stale BOOLEAN NOT NULL DEFAULT false,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    UNIQUE(tenant_id, academic_year, semester, student_id, subject_code)
);

-- Indeks performa tinggi untuk cetak rapor massal (O(1) lookups per rombel)
CREATE INDEX IF NOT EXISTS idx_gb_snapshot_lookup ON gradebook_snapshots(tenant_id, class_id, subject_code);
CREATE INDEX IF NOT EXISTS idx_gb_snapshot_student ON gradebook_snapshots(tenant_id, student_id, academic_year, semester);
CREATE INDEX IF NOT EXISTS idx_gb_snapshot_stale ON gradebook_snapshots(tenant_id, is_stale) WHERE is_stale = true;

-- Trigger updated_at
DROP TRIGGER IF EXISTS update_gradebook_snapshots_updated_at ON gradebook_snapshots;
CREATE TRIGGER update_gradebook_snapshots_updated_at
    BEFORE UPDATE ON gradebook_snapshots
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
