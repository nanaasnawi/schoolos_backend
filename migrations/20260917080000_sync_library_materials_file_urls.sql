-- 20260917080000_sync_library_materials_file_urls.sql
-- Ensure existing reading assignments have their external_url populated from library_books

UPDATE learning_materials m
SET external_url = lb.file_url
FROM library_books lb
WHERE m.library_book_id = lb.id
  AND (m.external_url IS NULL OR m.external_url = '');
