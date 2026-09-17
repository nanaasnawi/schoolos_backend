-- 20260917060000_seed_curriculum_library_books.sql
-- Seed standard Kurikulum Merdeka textbooks into library_books for all tenants

INSERT INTO library_books (id, tenant_id, title, author, publisher, subject_id, total_pages, cover_url, file_url, created_at, updated_at)
SELECT
    gen_random_uuid(),
    t.id,
    v.title,
    v.author,
    v.publisher,
    (SELECT s.id FROM subjects s WHERE s.tenant_id = t.id AND s.name ILIKE v.subject_pattern LIMIT 1),
    v.total_pages,
    v.cover_url,
    v.file_url,
    NOW(),
    NOW()
FROM tenants t
CROSS JOIN (
    VALUES
    (
        'Buku Siswa: Matematika Kelas X (Fase E Kurikulum Merdeka)',
        'Dicky Susanto, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%Matematika%',
        280,
        'https://images.unsplash.com/photo-1509228468518-180dd4864904?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-matematika-kelas-x'
    ),
    (
        'Buku Siswa: Bahasa Indonesia — Cerdas Cergas Berbahasa Indonesia Kelas X',
        'Fadillah Tri Aulia, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%Bahasa Indonesia%',
        252,
        'https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-bahasa-indonesia-kelas-x'
    ),
    (
        'Buku Siswa: Fisika untuk SMA/MA Kelas XI (Fase F)',
        'Marianna Magdalena Radjawane, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%IPA%',
        236,
        'https://images.unsplash.com/photo-1636466497217-26a8cbeaf0aa?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-fisika-kelas-xi'
    ),
    (
        'Buku Siswa: Biologi untuk SMA/MA Kelas XI (Fase F)',
        'Rini Solihat, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%IPA%',
        268,
        'https://images.unsplash.com/photo-1532094349884-543bc11b234d?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-biologi-kelas-xi'
    ),
    (
        'Buku Siswa: Kimia untuk SMA/MA Kelas XI (Fase F)',
        'Munasprianto Ramli, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%IPA%',
        248,
        'https://images.unsplash.com/photo-1603126857599-f6e157fa2fe6?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-kimia-kelas-xi'
    ),
    (
        'Buku Siswa: Bahasa Inggris — Work in Progress Kelas X',
        'Budi Hermawan, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%Bahasa Inggris%',
        184,
        'https://images.unsplash.com/photo-1544716278-ca5e3f4abd8c?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-bahasa-inggris-kelas-x'
    ),
    (
        'Buku Siswa: Informatika Kelas X',
        'Mushthofa, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%Komputer%',
        276,
        'https://images.unsplash.com/photo-1526374965328-7f61d4dc18c5?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-informatika-kelas-x'
    ),
    (
        'Buku Siswa: Pendidikan Pancasila Kelas X',
        'Rochimudin, dkk.',
        'Pusat Kurikulum dan Perbukuan Kemendikbudristek',
        '%Pancasila%',
        208,
        'https://images.unsplash.com/photo-1541829070764-84a7d30dd3f3?w=400&q=80',
        'https://buku.kemdikbud.go.id/katalog/buku-pendidikan-pancasila-kelas-x'
    )
) AS v(title, author, publisher, subject_pattern, total_pages, cover_url, file_url)
WHERE NOT EXISTS (
    SELECT 1 FROM library_books lb
    WHERE lb.tenant_id = t.id AND lb.title = v.title
);
