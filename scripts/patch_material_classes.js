const fs = require('fs');
const path = require('path');
const envText = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;
const { Client } = require('pg');
const client = new Client({ connectionString: dbUrl });

(async () => {
  await client.connect();
  console.log('Connected to database.');

  // Map each material to its proper class and subject
  const updates = [
    {
      match: '%Cerdas Cergas Berbahasa Indonesia Kelas X%',
      className: 'PAKET C 10',
      subjectName: 'Bahasa Indonesia'
    },
    {
      match: '%Bergerak Bersama untuk SD/MI Kelas V%',
      className: 'PAKET A 5',
      subjectName: 'Bahasa Indonesia'
    },
    {
      match: '%Matematika untuk SMP/MTs Kelas VII%',
      className: 'PAKET B 7',
      subjectName: 'Matematika (Umum)'
    },
    {
      match: '%Matematika untuk SMP/MTs Kelas IX%',
      className: 'PAKET B 9A',
      subjectName: 'Matematika (Umum)'
    },
    {
      match: '%Fisika untuk SMA/MA Kelas XI%',
      className: 'PAKET C 11A',
      subjectName: 'IPA (Kimia, Fisika, Biologi)'
    },
    {
      match: '%Membiasakan Berfikir Kritis & Semangat Mencintai IPTEK%',
      className: 'PAKET C 10',
      subjectName: 'Pendidikan Agama Islam dan Budi Pekerti'
    },
    {
      match: '%Singkatan & Akronim%',
      className: 'PAKET A 4',
      subjectName: 'Bahasa Indonesia'
    },
    {
      match: '%Panduan Guru Pembelajaran Keterampilan Kehidupan Sehari-hari%',
      className: 'PAKET A 4',
      subjectName: 'Seni Budaya'
    },
    {
      match: '%Anak-anak yang Mengubah Dunia untuk SD/MI Kelas VI%',
      className: 'PAKET A 6',
      subjectName: 'Bahasa Indonesia'
    },
    {
      match: '%BAB 2 Dibawah atap%',
      className: 'PAKET A 4',
      subjectName: 'Bahasa Indonesia'
    }
  ];

  for (const item of updates) {
    const classRes = await client.query('SELECT id FROM classes WHERE name = $1 LIMIT 1', [item.className]);
    const classId = classRes.rows[0]?.id;

    const subRes = await client.query('SELECT id FROM subjects WHERE name ILIKE $1 LIMIT 1', [`%${item.subjectName}%`]);
    const subjectId = subRes.rows[0]?.id;

    if (classId) {
      const res = await client.query(`
        UPDATE learning_materials 
        SET class_id = $1, 
            subject_id = COALESCE($2, subject_id) 
        WHERE title ILIKE $3 AND deleted_at IS NULL
      `, [classId, subjectId || null, item.match]);
      console.log(`Updated "${item.match}" -> class: ${item.className} (${classId}), subject: ${item.subjectName} (${subjectId}): ${res.rowCount} row(s)`);
    } else {
      console.warn(`Class not found: ${item.className}`);
    }
  }

  // Print updated materials
  const check = await client.query(`
    SELECT m.id, m.title, c.name as class_name, s.name as subject_name, u.full_name as teacher_name
    FROM learning_materials m
    LEFT JOIN classes c ON c.id = m.class_id
    LEFT JOIN subjects s ON s.id = m.subject_id
    LEFT JOIN teachers t ON t.id = m.teacher_id
    LEFT JOIN users u ON u.id = t.user_id
    WHERE m.deleted_at IS NULL
    ORDER BY m.created_at DESC
  `);
  console.log('\n--- VERIFICATION OF UPDATED MATERIALS ---');
  console.table(check.rows);

  await client.end();
})();
