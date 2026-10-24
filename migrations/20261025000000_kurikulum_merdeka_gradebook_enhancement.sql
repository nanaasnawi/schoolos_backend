-- ============================================================================
-- Migration: 20261025000000_kurikulum_merdeka_gradebook_enhancement.sql
-- Description: Standardize Gradebook schema for Kurikulum Merdeka (Kemendikdasmen RI).
--              - Eliminates numerical formative weighting in favor of qualitative reflection
--              - Adds Sumatif Modul average, optional ATS and SAS scores
--              - Adds KKTP passing threshold and automatic Capaian Kompetensi descriptors
-- ============================================================================

ALTER TABLE gradebooks
    ADD COLUMN IF NOT EXISTS avg_sumatif NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS ats_score NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS sas_score NUMERIC(5,2),
    ADD COLUMN IF NOT EXISTS kktp NUMERIC(5,2) DEFAULT 75.0,
    ADD COLUMN IF NOT EXISTS highest_competency_desc TEXT,
    ADD COLUMN IF NOT EXISTS lowest_competency_desc TEXT,
    ADD COLUMN IF NOT EXISTS formatif_notes TEXT;

CREATE INDEX IF NOT EXISTS idx_gradebooks_tenant_class_subject ON gradebooks(tenant_id, class_id, subject_id);
