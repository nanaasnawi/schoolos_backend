-- 20261010170000_create_academic_calendar_events.sql
-- Kurikulum Merdeka: Academic Calendar (Kaldik) & MEB Engine Schema

CREATE TABLE IF NOT EXISTS academic_calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL CHECK (semester IN ('ODD', 'EVEN')),
    title VARCHAR(255) NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    category VARCHAR(50) NOT NULL DEFAULT 'SCHOOL_EVENT',
    color VARCHAR(30) DEFAULT '#0284c7',
    description TEXT,
    is_effective_learning BOOLEAN NOT NULL DEFAULT false,
    is_national_holiday BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX IF NOT EXISTS idx_kaldik_tenant_year ON academic_calendar_events(tenant_id, academic_year);
CREATE INDEX IF NOT EXISTS idx_kaldik_dates ON academic_calendar_events(start_date, end_date);
CREATE INDEX IF NOT EXISTS idx_kaldik_semester ON academic_calendar_events(semester);

-- Seed National/Official Kaldik Kemendikdasmen 2026/2027 Baseline Events (tenant_id NULL for global template)
INSERT INTO academic_calendar_events 
    (tenant_id, academic_year, semester, title, start_date, end_date, category, color, description, is_effective_learning, is_national_holiday)
VALUES
    -- Semester Ganjil 2026/2027
    (NULL, '2026/2027', 'ODD', 'Masa Pengenalan Lingkungan Sekolah (MPLS) Ramah Anak', '2026-07-13', '2026-07-15', 'MPLS', '#8b5cf6', 'Orientasi peserta didik baru dan penguatan budaya sekolah sehat & ceria.', false, false),
    (NULL, '2026/2027', 'ODD', 'Hari Pertama Efektif Belajar Semester Ganjil', '2026-07-16', '2026-07-16', 'LEARNING_DAY', '#10b981', 'Awal kegiatan belajar mengajar (KBM) efektif Kurikulum Merdeka Tahun Ajaran 2026/2027.', true, false),
    (NULL, '2026/2027', 'ODD', 'HUT Kemerdekaan Republik Indonesia ke-81', '2026-08-17', '2026-08-17', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional memperingati Hari Kemerdekaan RI & Upacara Bendera.', false, true),
    (NULL, '2026/2027', 'ODD', 'Maulid Nabi Muhammad SAW 1448 H', '2026-08-25', '2026-08-25', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional peringatan hari besar keagamaan.', false, true),
    (NULL, '2026/2027', 'ODD', 'Sumatif Tengah Semester (STS) / PTS Ganjil', '2026-09-21', '2026-09-26', 'EXAM', '#f59e0b', 'Evaluasi formatif & sumatif pencapaian Tujuan Pembelajaran (TP) tengah semester.', false, false),
    (NULL, '2026/2027', 'ODD', 'Pelaksanaan Asesmen Nasional (ANBK) Gelombang 1', '2026-10-19', '2026-10-22', 'EXAM', '#f59e0b', 'Evaluasi sistem pemetaan mutu pendidikan nasional oleh Kemendikdasmen.', false, false),
    (NULL, '2026/2027', 'ODD', 'Hari Guru Nasional (HGN) & Peringatan PGRI', '2026-11-25', '2026-11-25', 'SCHOOL_EVENT', '#0284c7', 'Apresiasi dedikasi guru dan penguatan kapasitas pedagogis sekolah.', false, false),
    (NULL, '2026/2027', 'ODD', 'Sumatif Akhir Semester (SAS) / PAS Ganjil', '2026-11-30', '2026-12-11', 'EXAM', '#f59e0b', 'Penilaian sumatif akhir seluruh lingkup materi semester ganjil.', false, false),
    (NULL, '2026/2027', 'ODD', 'Pengolahan Nilai & Rapat Pleno Dewan Guru', '2026-12-14', '2026-12-18', 'SCHOOL_EVENT', '#0284c7', 'Finalisasi nilai e-Rapor dan persiapan penyerahan hasil belajar.', false, false),
    (NULL, '2026/2027', 'ODD', 'Pembagian Buku Rapor Semester Ganjil', '2026-12-19', '2026-12-19', 'REPORT_CARD', '#059669', 'Penyerahan laporan capaian hasil belajar peserta didik kepada orang tua/wali.', false, false),
    (NULL, '2026/2027', 'ODD', 'Libur Akhir Semester Ganjil & Libur Hari Raya Natal', '2026-12-21', '2027-01-02', 'HOLIDAY_SEMESTER', '#dc2626', 'Libur jeda semester 1 dan perayaan tahun baru 2027.', false, true),

    -- Semester Genap 2026/2027
    (NULL, '2026/2027', 'EVEN', 'Hari Pertama Masuk Sekolah Semester Genap', '2027-01-04', '2027-01-04', 'LEARNING_DAY', '#10b981', 'Awal kegiatan belajar mengajar (KBM) efektif semester 2.', true, false),
    (NULL, '2026/2027', 'EVEN', 'Isra Mi''raj Nabi Muhammad SAW 1448 H', '2027-01-27', '2027-01-27', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional peringatan hari besar keagamaan.', false, true),
    (NULL, '2026/2027', 'EVEN', 'Tahun Baru Imlek 2578 Kongzili', '2027-02-06', '2027-02-06', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional.', false, true),
    (NULL, '2026/2027', 'EVEN', 'Libur Awal Ramadhan 1448 H', '2027-03-08', '2027-03-10', 'SCHOOL_EVENT', '#0284c7', 'Penyesuaian jam belajar dan pesantren kilat Ramadhan.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Sumatif Tengah Semester (STS) Genap', '2027-03-15', '2027-03-20', 'EXAM', '#f59e0b', 'Evaluasi formatif & sumatif tengah semester genap.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Ujian Sekolah Praktik Kelas Akhir', '2027-03-22', '2027-03-27', 'EXAM', '#f59e0b', 'Ujian praktik mata pelajaran kompetensi keahlian dan seni budaya.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Libur Hari Raya Idul Fitri 1448 H & Cuti Bersama', '2027-03-29', '2027-04-03', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Hari Raya Idul Fitri dan cuti bersama nasional.', false, true),
    (NULL, '2026/2027', 'EVEN', 'Ujian Satuan Pendidikan / PSAJ Kelas Akhir', '2027-04-05', '2027-04-10', 'EXAM', '#f59e0b', 'Penilaian Sumatif Akhir Jenjang untuk penentuan kelulusan siswa.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Hari Pendidikan Nasional (Hardiknas)', '2027-05-02', '2027-05-02', 'SCHOOL_EVENT', '#0284c7', 'Upacara bendera & peringatan Hari Pendidikan Nasional.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Kenaikan Isa Almasih', '2027-05-06', '2027-05-06', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional.', false, true),
    (NULL, '2026/2027', 'EVEN', 'Hari Raya Waisak 2571', '2027-05-20', '2027-05-20', 'NATIONAL_HOLIDAY', '#ef4444', 'Libur Nasional.', false, true),
    (NULL, '2026/2027', 'EVEN', 'Sumatif Akhir Tahun (SAT) / PAT Genap', '2027-05-31', '2027-06-11', 'EXAM', '#f59e0b', 'Penilaian sumatif penentuan kenaikan kelas semester genap.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Pengolahan Nilai Kenaikan Kelas & Rapat Kelulusan', '2026-06-14', '2026-06-18', 'SCHOOL_EVENT', '#0284c7', 'Sidang dewan guru untuk penetapan kenaikan kelas dan kelulusan.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Pembagian Rapor Semester Genap & Kelulusan', '2027-06-19', '2027-06-19', 'REPORT_CARD', '#059669', 'Penyerahan buku rapor kenaikan kelas dan ijazah/SKL kelulusan.', false, false),
    (NULL, '2026/2027', 'EVEN', 'Libur Akhir Tahun Ajaran 2026/2027', '2027-06-21', '2027-07-10', 'HOLIDAY_SEMESTER', '#dc2626', 'Libur kenaikan kelas dan transisi Tahun Ajaran baru 2027/2028.', false, true)
ON CONFLICT DO NOTHING;
