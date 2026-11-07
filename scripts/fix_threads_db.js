const { Client } = require('pg');
const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway'
});

async function run() {
  await client.connect();

  // 1. Restore 5ec494a8 to Taufiq Hidayat
  await client.query(`
    UPDATE inquiry_threads
    SET subject_name = 'Geografi',
        reference_title = 'Tugas Siswa: 5 soal pilihan ganda dan uraian',
        last_message_content = 'Halo TAUFIQ HIDAYAT, saya ingin berkonsultasi mengenai tugas ''Tugas Siswa: 5 soal pilihan ganda dan uraian'' ini karena ada bagian yang belum saya pahami:',
        last_message_at = '2026-11-07 05:06:51.363+00',
        updated_at = '2026-11-07 05:06:51.363+00'
    WHERE id = '5ec494a8-cf63-4b4d-beec-e397ff1a7b37'
  `);
  console.log('Restored thread 5ec494a8 to Taufiq Hidayat');

  // 2. Find or create a dedicated Principal consultation thread for Renaldi
  const existingPrinThread = await client.query(`
    SELECT id FROM inquiry_threads
    WHERE tenant_id = 'b158ec8a-6ea3-468a-bed5-917e37499550'
      AND student_name = 'RENALDI'
      AND (teacher_name ILIKE '%kepala%' OR subject_name ILIKE '%kebijakan%')
  `);

  let prinThreadId;
  if (existingPrinThread.rows.length > 0) {
    prinThreadId = existingPrinThread.rows[0].id;
    console.log('Found existing principal thread:', prinThreadId);
  } else {
    // Generate new UUID for principal thread
    const newTh = await client.query(`
      INSERT INTO inquiry_threads (
        id, tenant_id, student_id, student_name, student_class,
        teacher_id, teacher_name, subject_name, inquiry_type,
        reference_title, reference_id, status, last_message_content,
        last_message_at, created_at, updated_at
      ) VALUES (
        gen_random_uuid(),
        'b158ec8a-6ea3-468a-bed5-917e37499550',
        '01a096e3-d808-7650-918d-66c4d7aed825',
        'RENALDI',
        'PAKET C 12B',
        '01a092fc-af59-76b2-85f2-cca8b8d8d515',
        'Kepala Sekolah PKBM',
        'Konsultasi Kebijakan, Akademik & Pengaduan Warga Belajar',
        'GENERAL',
        'Konsultasi Kebijakan, Akademik & Pengaduan Warga Belajar',
        NULL,
        'WAITING_REPLY',
        'test',
        '2026-11-07 06:47:34.646+00',
        '2026-11-07 06:47:34.646+00',
        '2026-11-07 06:47:34.646+00'
      ) RETURNING id
    `);
    prinThreadId = newTh.rows[0].id;
    console.log('Created dedicated principal thread:', prinThreadId);
  }

  // 3. Move message c1814382 to prinThreadId
  await client.query(`
    UPDATE inquiry_messages
    SET thread_id = $1
    WHERE id = 'c1814382-3d5f-49de-8d30-1532bcb3f70e'
  `, [prinThreadId]);
  console.log('Moved test message to principal thread', prinThreadId);

  await client.end();
}

run().catch(console.error);

