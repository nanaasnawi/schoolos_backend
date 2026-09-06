const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgres://school_admin:secretpassword@localhost:5433/school_os'
});

const MAPPINGS = [
  { className: 'PAKET A4', teacherName: 'ESI ROKESI' },
  { className: 'PAKET A5', teacherName: 'FITRI NAFISAH' },
  { className: 'PAKET A6', teacherName: 'KHAERIYAH' },
  { className: 'PAKET B7', teacherName: 'KRISTIANTI' },
  { className: 'PAKET B8', teacherName: 'ASEP RIFAI' },
  { className: 'PAKET B8a', teacherName: 'ASEP RIFAI' },
  { className: 'PAKET B8b', teacherName: 'ASY SYIFA RAHMAH IHSANI' },
  { className: 'PAKET B9', teacherName: 'KUSWANTO ADI WIJAYA' },
  { className: 'PAKET C10', teacherName: 'EHA MEIDA KARTIKA' },
  { className: 'PAKET C11a', teacherName: 'AMIN LISANA' },
  { className: 'PAKET C11b', teacherName: 'ROHMANA' },
  { className: 'PAKET C12a', teacherName: 'SRI MULYANI.S.AG' },
  { className: 'PAKET C12b', teacherName: 'TAUFIQ HIDAYAT' },
  { className: 'KKA C11 1', teacherName: 'AMIN LISANA' },
  { className: 'KKA C11 2', teacherName: 'ASEP RIFAI' },
  { className: 'KKA C11 3', teacherName: 'EHA MEIDA KARTIKA' },
  { className: 'KKA C11 4', teacherName: 'ROHMANA' },
  { className: 'KKA C12 1', teacherName: 'SRI MULYANI.S.AG' },
  { className: 'KKA C12 2', teacherName: 'TAUFIQ HIDAYAT' },
  { className: 'KKA C12 3', teacherName: 'KUSWANTO ADI WIJAYA' },
  { className: 'KKA C12 4', teacherName: 'KRISTIANTI' },
];

async function main() {
  await client.connect();
  console.log('Connected to PostgreSQL...');

  const teachersRes = await client.query('SELECT id, full_name FROM teachers');
  const teacherMap = new Map();
  teachersRes.rows.forEach(t => {
    teacherMap.set(t.full_name.toUpperCase(), t.id);
  });

  let updatedCount = 0;
  for (const m of MAPPINGS) {
    const teacherId = teacherMap.get(m.teacherName.toUpperCase());
    if (teacherId) {
      const res = await client.query(
        'UPDATE classes SET homeroom_teacher_id = $1, updated_at = NOW() WHERE name = $2',
        [teacherId, m.className]
      );
      if (res.rowCount > 0) {
        updatedCount += res.rowCount;
      }
    } else {
      console.warn(`Teacher not found: ${m.teacherName}`);
    }
  }

  console.log(`Successfully mapped homeroom teachers for ${updatedCount} classes!`);

  // Verify result
  const verify = await client.query(`
    SELECT c.name, t.full_name as wali_kelas
    FROM classes c
    LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id
    ORDER BY c.name ASC
  `);
  console.table(verify.rows);

  await client.end();
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
