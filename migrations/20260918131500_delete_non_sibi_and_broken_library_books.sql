-- 20260918131500_delete_non_sibi_and_broken_library_books.sql
-- 1. Unlink any learning materials that reference broken/withdrawn/non-PDF books
UPDATE learning_materials
SET library_book_id = NULL,
    external_url = NULL
WHERE library_book_id IN (
    '33ec53ab-f130-4a9b-9501-737501aefb8a',
    '1834a10c-e102-4e25-b67b-01f7befdbb23',
    '69bf1172-83af-4b7c-a315-36d20bd385fe',
    '057c7140-9452-400e-83c9-57dc584b09bd',
    '6c3e1b7a-9524-444d-b0c1-e8b10ef591a0',
    '359beced-cd37-4db0-aa30-d1d4782ef473',
    'c0e97290-cd6a-40c4-80a1-f58f1be05ef0',
    '51f8c1fb-1d69-4c24-882a-2753cd516dd0',
    '74a3540c-9811-4785-81c2-499c2920d3c8',
    '6f8e15c6-86bd-4ebe-9268-3b7be372bde6',
    '0afe60fc-1cc1-4d94-9372-138c7b5a87a6',
    '6d8781cc-8220-4bd0-8689-e065cbbddac5',
    '4c039673-7b21-4540-8cec-1bd7fc43de27',
    'c3af517f-cc64-4ed4-bd1e-02f3dc071d0f',
    '4a2fd7b7-2258-4373-9813-eb88b62abdfc',
    'e8f71912-bc07-4a2c-a158-324aa5ad5ccd',
    '4b17a40b-f00b-451a-be8b-66ab75189910',
    '161dc540-a5e3-418b-8c07-391c167e2097',
    '8eacbc15-ddb6-4cfe-89fa-91705d8dbf6e',
    '10e9ea52-3aa1-41fd-af06-890cdce2f3b2',
    '6ab13684-852b-4239-b181-89addbf5f8d5',
    'febb2edc-b913-4178-9405-f958ed0b2240',
    '03dbf9f0-690f-4b0c-8bd1-c1794f555310',
    'f9bf8c44-a661-41a7-a8cc-66b3be4d0972',
    'ad51debe-a17a-45d6-bc41-557ab622f834',
    '0bce9d75-3d70-4285-90d9-29e123c7917d',
    'bc5da637-2b38-4b85-836c-efc97645d04d',
    '046c1a9d-4c36-4912-841c-03b22cf0900c',
    'd96bdc2d-7da4-4010-8f30-bfc19a50e9a7',
    'f3e87098-3593-47a7-b828-cc825b14275a',
    'c7f61aed-38e4-48cb-b8ea-24ad4ecb236b',
    'ef7a4b19-cb7b-45e6-8e6c-34142dba5de4',
    'b1757f60-40a1-49de-a09a-dd5f59712785',
    '97f72b72-176a-45ef-bd9b-a30286d0d85b',
    '847c744a-e4f7-4ca7-83f1-2fb6abafb235',
    'c227df43-73d7-4617-a6c6-e4d015d6a914',
    'bcb4eb5d-e896-479e-a7eb-3fe57e327090',
    'e3e80ce5-33ab-40d3-b2cf-a3925a61695b',
    '26ab4e8c-3bc9-477e-9d21-e40e72696278',
    '40e2626d-09d2-474c-9057-17e1268f6eb6',
    '61265869-122f-491e-84b4-48a1bb4304c5',
    '35f27ac5-95cf-4e0d-8b1a-857274983ecb',
    '85668696-40eb-4134-b779-0518f8291ee6',
    'dc621a51-272e-4eed-b569-13a1ffdf4c79',
    'd4de85ab-084c-495a-8174-6805fdc63ef9',
    'f4fd0935-7331-4420-aac9-2cb929396b80',
    'ef2500c5-c929-4c8d-9f8d-8c74392e634d',
    'c0fda71f-7b7a-4350-86c9-8152b73f849a',
    '90357bb7-313c-4086-932e-6d43474eaab1',
    '752a1131-3641-44f2-860d-eb25d8d43e16'
);

-- 2. Delete the 50 invalid/broken/non-PDF/withdrawn books from library_books
DELETE FROM library_books
WHERE id IN (
    '33ec53ab-f130-4a9b-9501-737501aefb8a',
    '1834a10c-e102-4e25-b67b-01f7befdbb23',
    '69bf1172-83af-4b7c-a315-36d20bd385fe',
    '057c7140-9452-400e-83c9-57dc584b09bd',
    '6c3e1b7a-9524-444d-b0c1-e8b10ef591a0',
    '359beced-cd37-4db0-aa30-d1d4782ef473',
    'c0e97290-cd6a-40c4-80a1-f58f1be05ef0',
    '51f8c1fb-1d69-4c24-882a-2753cd516dd0',
    '74a3540c-9811-4785-81c2-499c2920d3c8',
    '6f8e15c6-86bd-4ebe-9268-3b7be372bde6',
    '0afe60fc-1cc1-4d94-9372-138c7b5a87a6',
    '6d8781cc-8220-4bd0-8689-e065cbbddac5',
    '4c039673-7b21-4540-8cec-1bd7fc43de27',
    'c3af517f-cc64-4ed4-bd1e-02f3dc071d0f',
    '4a2fd7b7-2258-4373-9813-eb88b62abdfc',
    'e8f71912-bc07-4a2c-a158-324aa5ad5ccd',
    '4b17a40b-f00b-451a-be8b-66ab75189910',
    '161dc540-a5e3-418b-8c07-391c167e2097',
    '8eacbc15-ddb6-4cfe-89fa-91705d8dbf6e',
    '10e9ea52-3aa1-41fd-af06-890cdce2f3b2',
    '6ab13684-852b-4239-b181-89addbf5f8d5',
    'febb2edc-b913-4178-9405-f958ed0b2240',
    '03dbf9f0-690f-4b0c-8bd1-c1794f555310',
    'f9bf8c44-a661-41a7-a8cc-66b3be4d0972',
    'ad51debe-a17a-45d6-bc41-557ab622f834',
    '0bce9d75-3d70-4285-90d9-29e123c7917d',
    'bc5da637-2b38-4b85-836c-efc97645d04d',
    '046c1a9d-4c36-4912-841c-03b22cf0900c',
    'd96bdc2d-7da4-4010-8f30-bfc19a50e9a7',
    'f3e87098-3593-47a7-b828-cc825b14275a',
    'c7f61aed-38e4-48cb-b8ea-24ad4ecb236b',
    'ef7a4b19-cb7b-45e6-8e6c-34142dba5de4',
    'b1757f60-40a1-49de-a09a-dd5f59712785',
    '97f72b72-176a-45ef-bd9b-a30286d0d85b',
    '847c744a-e4f7-4ca7-83f1-2fb6abafb235',
    'c227df43-73d7-4617-a6c6-e4d015d6a914',
    'bcb4eb5d-e896-479e-a7eb-3fe57e327090',
    'e3e80ce5-33ab-40d3-b2cf-a3925a61695b',
    '26ab4e8c-3bc9-477e-9d21-e40e72696278',
    '40e2626d-09d2-474c-9057-17e1268f6eb6',
    '61265869-122f-491e-84b4-48a1bb4304c5',
    '35f27ac5-95cf-4e0d-8b1a-857274983ecb',
    '85668696-40eb-4134-b779-0518f8291ee6',
    'dc621a51-272e-4eed-b569-13a1ffdf4c79',
    'd4de85ab-084c-495a-8174-6805fdc63ef9',
    'f4fd0935-7331-4420-aac9-2cb929396b80',
    'ef2500c5-c929-4c8d-9f8d-8c74392e634d',
    'c0fda71f-7b7a-4350-86c9-8152b73f849a',
    '90357bb7-313c-4086-932e-6d43474eaab1',
    '752a1131-3641-44f2-860d-eb25d8d43e16'
);

-- 3. Also delete any non-PDF file_urls or interactive/partner links that might exist
DELETE FROM library_books
WHERE file_url IS NULL 
   OR file_url NOT LIKE '%.pdf'
   OR file_url LIKE '%pesonaedu%'
   OR file_url LIKE '%buku_elektronik_kurmer%';

-- 4. Clean up tenant-specific duplicates where a global (tenant_id IS NULL) version already exists
DELETE FROM library_books lb
WHERE lb.tenant_id IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM library_books global_lb
    WHERE global_lb.tenant_id IS NULL
      AND global_lb.title = lb.title
  );
