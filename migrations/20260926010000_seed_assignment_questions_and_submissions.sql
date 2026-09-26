-- 20260926010000_seed_assignment_questions_and_submissions.sql

UPDATE assignments
SET instructions = 'Kerjakan seluruh soal Pilihan Ganda dan Esai di bawah ini dengan teliti. Pastikan setiap jawaban telah terisi sebelum menekan tombol Kumpulkan Tugas.'
WHERE id = 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8';

INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
VALUES ('316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0', 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8', 'Manakah di bawah ini yang merupakan kelompok besaran pokok menurut Sistem Internasional (SI)?', 'MULTIPLE_CHOICE', 20, 1)
ON CONFLICT (id) DO UPDATE SET question_text = EXCLUDED.question_text, points = EXCLUDED.points;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('e2ed5724-8d5e-42ca-923e-f0d51c745faa', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'Kecepatan, massa, waktu, dan luas', false, 1)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('eb9f3409-7897-4509-8de1-81ee2e9fe029', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'Panjang, massa, waktu, dan suhu', true, 2)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('33d5de76-32c7-49e7-82e4-78deefdfd7b1', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'Gaya, percepatan, kuat arus, dan volume', false, 3)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('2a0bf35e-cd0f-481c-9eb8-1fe2eb5190c4', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'Massa jenis, panjang, daya, dan intensitas cahaya', false, 4)
ON CONFLICT (id) DO NOTHING;

INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
VALUES ('9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0', 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8', 'Alat ukur yang memiliki tingkat ketelitian mencapai 0,01 mm dan paling tepat digunakan untuk mengukur ketebalan selembar kertas atau kawat adalah...', 'MULTIPLE_CHOICE', 20, 2)
ON CONFLICT (id) DO UPDATE SET question_text = EXCLUDED.question_text, points = EXCLUDED.points;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('fd90812f-6461-474d-9096-b35ff0f38824', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', 'Mistar ukur baja', false, 1)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('08bef70b-d476-4ef1-a76c-0a9f154ad734', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', 'Jangka sorong', false, 2)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('20fb7d79-03ba-4cbb-814d-1c360912a507', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', 'Mikrometer sekrup', true, 3)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('e8c1e78d-9299-4fd7-99e4-ab087dda6de7', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', 'Meteran gulung kain', false, 4)
ON CONFLICT (id) DO NOTHING;

INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
VALUES ('23ed09e5-5c66-4509-8731-d9a8268cf329', 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0', 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8', 'Satuan baku dalam Sistem Internasional (SI) untuk besaran pokok suhu dan intensitas cahaya berturut-turut adalah...', 'MULTIPLE_CHOICE', 20, 3)
ON CONFLICT (id) DO UPDATE SET question_text = EXCLUDED.question_text, points = EXCLUDED.points;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('0981a5ee-4d24-442b-bc2f-bee783763f33', '23ed09e5-5c66-4509-8731-d9a8268cf329', 'Celcius dan Candela', false, 1)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('5c58cf60-226e-4ce6-8d9e-13286bc809c5', '23ed09e5-5c66-4509-8731-d9a8268cf329', 'Kelvin dan Candela', true, 2)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('b9234119-573a-4626-81ff-054b5ace47cb', '23ed09e5-5c66-4509-8731-d9a8268cf329', 'Fahrenheit dan Watt', false, 3)
ON CONFLICT (id) DO NOTHING;
INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
VALUES ('72016676-1d7d-40cd-b32a-29f99edc33fc', '23ed09e5-5c66-4509-8731-d9a8268cf329', 'Reamur dan Lux', false, 4)
ON CONFLICT (id) DO NOTHING;

INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
VALUES ('cf01c590-9af6-44c9-976c-d3fd942a2c86', 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0', 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8', 'Jelaskan perbedaan mendasar antara besaran pokok dan besaran turunan! Berikan masing-masing 2 contoh besaran beserta satuannya dalam SI.', 'ESSAY', 20, 4)
ON CONFLICT (id) DO UPDATE SET question_text = EXCLUDED.question_text, points = EXCLUDED.points;

INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
VALUES ('caa126df-a3e4-4dac-b680-ccaf728f0bed', 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0', 'ae375f1e-e915-4ed4-8fe1-7ea519d39fe8', 'Berdasarkan pengamatan mandiri yang telah kamu lakukan di rumah/lingkungan sekitar, tuliskan hasil pengukuran 3 benda berbeda (nama benda, besaran yang diukur, alat ukur yang digunakan, dan hasil pengukuran beserta satuannya)!', 'ESSAY', 20, 5)
ON CONFLICT (id) DO UPDATE SET question_text = EXCLUDED.question_text, points = EXCLUDED.points;

INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('f763fcba-f61b-4080-9ebe-2c563a979fcf', '65319769-9936-401e-a446-520aea1a3788', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('b524481d-c751-4a2b-befd-84ca480722ce', '65319769-9936-401e-a446-520aea1a3788', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('8964b06a-1508-4f1f-aaae-1202ff1e6944', '65319769-9936-401e-a446-520aea1a3788', '23ed09e5-5c66-4509-8731-d9a8268cf329', '0981a5ee-4d24-442b-bc2f-bee783763f33', NULL, 0, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('73ac15de-77ce-42bf-9ab0-c3260c05105f', '65319769-9936-401e-a446-520aea1a3788', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 15, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('a265c030-6058-475f-9a3b-dd23e6f73317', '65319769-9936-401e-a446-520aea1a3788', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Berikut adalah hasil pengamatan dan analisis pengukuran 5 besaran pokok: 1. Panjang meja (1.20 m), 2. Massa buku cetak (0.45 kg), 3. Waktu osilasi bandul (1.42 s), 4. Kuat arus charger HP (2.0 A), 5. Suhu ruangan belajar (27.5 °C). Perhitungan konversi dan ketidakpastian pengukuran sudah dicantumkan dalam lembar kerja terlampir.', 22, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('8641b40e-5ce3-4f8b-9ebb-4429be8d5424', '585ebd89-6fad-4afd-b1c5-d654245e01df', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('ca8f0813-6f4b-4007-b407-9991fa7a7d0a', '585ebd89-6fad-4afd-b1c5-d654245e01df', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('1ab7fc9e-d4b5-484c-af14-5f8ecdd5d910', '585ebd89-6fad-4afd-b1c5-d654245e01df', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('aa163d45-ad12-4041-950e-b557cc2020fd', '585ebd89-6fad-4afd-b1c5-d654245e01df', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('1cb66e61-d9c7-403b-b476-4bd6e754aa6c', '585ebd89-6fad-4afd-b1c5-d654245e01df', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Tugas Analisis Praktik: Pengukuran Besaran Pokok telah diselesaikan. Pengukuran dilakukan dengan mistar, timbangan digital dapur, dan stopwatch smartphone. Semua data tabel beserta dokumentasi foto terlampir pada dokumen PDF.', 14, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('2ea5341b-d35a-4209-8a03-4172bd0243b8', '8c1250c5-252b-4042-bf6e-386692a92542', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('1ac83c8a-08d6-4198-bddb-28e17262a955', '8c1250c5-252b-4042-bf6e-386692a92542', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('85a43bef-d0d1-454e-acf4-101d6879d1a4', '8c1250c5-252b-4042-bf6e-386692a92542', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('91fd0b8b-4659-4b30-899a-1fe12bbfc906', '8c1250c5-252b-4042-bf6e-386692a92542', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('8248a16d-d85d-4c6c-a413-33194b32f616', '8c1250c5-252b-4042-bf6e-386692a92542', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Tugas fisika pengukuran besaran pokok: Telah mengukur meja belajar, botol minum, ketebalan modul, serta mencatat suhu air es dan mendidih. Lembar jawaban ditulis tangan dan difoto dengan jelas.', 18, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('8b2bf36f-c7ca-4e01-a346-a133b6b4fbc6', 'fffb62de-06f9-4912-9f67-4fe7d443287a', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('55bed768-3799-41e2-a815-65aa28ff34fe', 'fffb62de-06f9-4912-9f67-4fe7d443287a', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('891127c1-ad6d-464a-a646-5ad78fdbefd2', 'fffb62de-06f9-4912-9f67-4fe7d443287a', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('ae992588-96eb-41fe-9d86-1997c97a4b39', 'fffb62de-06f9-4912-9f67-4fe7d443287a', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('c765cd23-4d36-4e8c-b034-bffe8c89449f', 'fffb62de-06f9-4912-9f67-4fe7d443287a', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Hasil pengukuran besaran pokok dan besaran turunan. Disertai dengan analisis dimensi: [L] untuk panjang, [M] untuk massa, dan [T] untuk waktu. Grafik hubungan massa terhadap volume juga disertakan.', 18, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('c69dda58-95e8-48b0-9056-2c8821a022ad', '28ec95d1-c1c1-4af4-a429-20fa3e01f2c0', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('0284ccfa-4521-48b2-90ae-d79363b72132', '28ec95d1-c1c1-4af4-a429-20fa3e01f2c0', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('50016928-b097-477c-8229-bd814493f1d7', '28ec95d1-c1c1-4af4-a429-20fa3e01f2c0', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('84cad3d3-9fa0-45c0-a68e-17b33ab266a8', '28ec95d1-c1c1-4af4-a429-20fa3e01f2c0', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('65ce67f5-161f-451c-9fb1-119e010627eb', '28ec95d1-c1c1-4af4-a429-20fa3e01f2c0', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Lembar jawaban tugas pengukuran praktikum mandiri. Tabel 5 besaran pokok terisi lengkap dengan konversi ke satuan cgs dan mks.', 18, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('ecd8fd6d-a2a4-44c4-ae38-35f19cdcfbad', '4ca44330-58b7-40b4-b336-8c914614dde0', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('1cf6b818-9e31-4306-9d74-ecd21d6f3509', '4ca44330-58b7-40b4-b336-8c914614dde0', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('c17f5325-cb5b-4903-b32b-6c93d0cc6dad', '4ca44330-58b7-40b4-b336-8c914614dde0', '23ed09e5-5c66-4509-8731-d9a8268cf329', '0981a5ee-4d24-442b-bc2f-bee783763f33', NULL, 0, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('96263e86-6168-4a5e-8973-c426f9475d66', '4ca44330-58b7-40b4-b336-8c914614dde0', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 15, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('9413ff22-32f9-4136-8e0b-3ba0e18b5ed5', '4ca44330-58b7-40b4-b336-8c914614dde0', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Laporan pengamatan besaran pokok: Mistar ukur 30 cm, Stopwatch 1/100 s, Timbangan digital 0.1 g. Semua instrumen telah dikalibrasi awal nol sebelum pengambilan data.', 23, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('5285ab85-6547-4760-a307-319e2bf194d8', '056dac13-9388-4883-850f-ba3f2bbec1d3', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('87e02d0f-a87f-4279-b6d6-640c3d1318c4', '056dac13-9388-4883-850f-ba3f2bbec1d3', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('5e6af1a5-9945-44be-9413-3c97d82e4e22', '056dac13-9388-4883-850f-ba3f2bbec1d3', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('bad55346-7079-4206-8123-c713a4fe5180', '056dac13-9388-4883-850f-ba3f2bbec1d3', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('f0c0ab8f-1f7e-4a3d-b7ec-750b6363ee7c', '056dac13-9388-4883-850f-ba3f2bbec1d3', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Mohon izin mengumpulkan tugas analisis besaran pokok Pak Guru. Saya mengukur 5 benda di bengkel kerja: panjang balok kayu, diameter pipa PVC, berat mur baut, dan waktu pendinginan oli. File terlampir.', 18, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('ae254cbb-2db3-456c-9955-ce420b3ac613', 'e1e61996-ddb8-496c-a57e-55a865a078ae', '316ce80e-3cd5-43c3-835d-4deb5ac8ca06', 'eb9f3409-7897-4509-8de1-81ee2e9fe029', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('ea46867d-2fc9-4845-8429-8c3758caa26b', 'e1e61996-ddb8-496c-a57e-55a865a078ae', '9bdc701f-ebe5-4cb7-b0a5-4f48b8ffe70e', '20fb7d79-03ba-4cbb-814d-1c360912a507', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('a469e7e0-7de7-426d-bb0c-75371276f5fe', 'e1e61996-ddb8-496c-a57e-55a865a078ae', '23ed09e5-5c66-4509-8731-d9a8268cf329', '5c58cf60-226e-4ce6-8d9e-13286bc809c5', NULL, 20, NULL)
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('10db30f3-a974-42c5-b39b-48055ac9eb75', 'e1e61996-ddb8-496c-a57e-55a865a078ae', 'cf01c590-9af6-44c9-976c-d3fd942a2c86', NULL, 'Besaran pokok adalah besaran yang satuannya telah didefinisikan terlebih dahulu dan tidak diturunkan dari besaran lain (contoh: panjang [meter], massa [kilogram]). Besaran turunan adalah besaran yang diturunkan dari satu atau lebih besaran pokok (contoh: luas [m^2], kecepatan [m/s]).', 18, 'Penjelasan konsep sangat tepat dan contoh sudah sesuai standar SI.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
INSERT INTO assignment_submission_answers (id, submission_id, question_id, chosen_choice_id, text_answer, points_earned, teacher_feedback)
VALUES ('9c0403f7-71da-4a58-b276-9ad77aff252e', 'e1e61996-ddb8-496c-a57e-55a865a078ae', 'caa126df-a3e4-4dac-b680-ccaf728f0bed', NULL, 'Tugas pengukuran mandiri telah saya unggah dari Android App School OS. Mohon koreksi dan arahannya Bapak bila ada rumus yang belum tepat.', 18, 'Data pengamatan faktual dan konversi satuan ke SI sudah tepat.')
ON CONFLICT (submission_id, question_id) DO NOTHING;
