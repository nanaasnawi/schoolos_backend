const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgres://school_admin:secretpassword@localhost:5433/school_os'
});

async function main() {
  await client.connect();
  console.log('Connected to PostgreSQL...');

  const TENANT_ID = 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0'; // PKBM As-Salafiyah

  // 1. Get Academic Year
  const yearRes = await client.query('SELECT id, name FROM academic_years WHERE tenant_id = $1 LIMIT 1', [TENANT_ID]);
  const academicYearId = yearRes.rows.length > 0 ? yearRes.rows[0].id : null;
  console.log(`Using Academic Year ID: ${academicYearId}`);

  // 2. Ensure new teachers exist
  const newTeachers = [
    { name: 'SUSILAWATI', subject: 'Pendidikan Pancasila' },
    { name: 'AMELIANI DWI IRA FANISA', subject: 'Matematika' },
    { name: 'HUSNUL CHATIM', subject: 'Seni Budaya' }
  ];

  for (const t of newTeachers) {
    const existing = await client.query(
      'SELECT id FROM teachers WHERE tenant_id = $1 AND UPPER(full_name) = $2',
      [TENANT_ID, t.name.toUpperCase()]
    );
    if (existing.rows.length === 0) {
      await client.query(`
        INSERT INTO teachers (id, tenant_id, full_name, subject, is_active, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, true, NOW(), NOW())
      `, [TENANT_ID, t.name, t.subject]);
      console.log(`+ Added new teacher: ${t.name}`);
    } else {
      console.log(`✓ Teacher already exists: ${t.name}`);
    }
  }

  // Build teacher map (UPPER(full_name) -> id)
  const allTeachersRes = await client.query('SELECT id, UPPER(full_name) as name FROM teachers WHERE tenant_id = $1', [TENANT_ID]);
  const teacherMap = new Map();
  allTeachersRes.rows.forEach(r => teacherMap.set(r.name, r.id));

  // Helper to find teacher ID with aliases
  function getTeacherId(name) {
    const upper = name.toUpperCase();
    if (teacherMap.has(upper)) return teacherMap.get(upper);

    // Aliases
    if (upper === "ASEP RIFA'I" || upper === 'ASEF RIFAI') return teacherMap.get('ASEP RIFAI');
    if (upper === 'ASYIFA R.I' || upper === 'AS SYIFA RI') return teacherMap.get('ASY SYIFA RAHMAH IHSANI');
    if (upper === 'TAUFIK HIDAYAT') return teacherMap.get('TAUFIQ HIDAYAT');
    if (upper === 'KUSWANTO') return teacherMap.get('KUSWANTO ADI WIJAYA');
    if (upper === 'FITRI NAPISAH') return teacherMap.get('FITRI NAFISAH');
    if (upper === 'SRI MULYANI') return teacherMap.get('SRI MULYANI.S.AG');

    // Partial match fallback
    for (const [k, v] of teacherMap.entries()) {
      if (k.includes(upper) || upper.includes(k)) return v;
    }
    throw new Error(`Teacher not found: ${name}`);
  }

  // 3. Ensure all subjects exist
  const subjectsToEnsure = [
    { code: '100011070', name: 'Pendidikan Agama Islam dan Budi Pekerti' },
    { code: '200010300', name: 'Pendidikan Pancasila' },
    { code: '300110000', name: 'Bahasa Indonesia' },
    { code: '401000000', name: 'Matematika (Umum)' },
    { code: '401900000', name: 'Ilmu Pengetahuan Alam dan Sosial (IPAS)' },
    { code: '300210000', name: 'Bahasa Inggris' },
    { code: '500010000', name: 'Pendidikan Jasmani, Olahraga, dan Kesehatan' },
    { code: '843020100', name: 'Seni Budaya' },
    { code: '700201090', name: 'Keterampilan Komputer' },
    { code: '401200000', name: 'Ilmu Pengetahuan Sosial (IPS)' },
    { code: '300311901', name: 'Bahasa Cirebon' },
    { code: '401100000', name: 'Ilmu Pengetahuan Alam (IPA)' },
    { code: '401230000', name: 'Sejarah' },
    { code: '401220000', name: 'Geografi' },
    { code: '401240000', name: 'Sosiologi' },
    { code: '401210000', name: 'Ekonomi' },
    { code: '401110000', name: 'IPA (Kimia, Fisika, Biologi)' }
  ];

  for (const s of subjectsToEnsure) {
    await client.query(`
      INSERT INTO subjects (id, tenant_id, code, name, is_active, created_at, updated_at)
      VALUES (gen_random_uuid(), $1, $2, $3, true, NOW(), NOW())
      ON CONFLICT (tenant_id, code) DO UPDATE
      SET name = EXCLUDED.name, updated_at = NOW()
    `, [TENANT_ID, s.code, s.name]);
  }
  console.log(`✓ All ${subjectsToEnsure.length} subjects ensured.`);

  // Build subject map (name -> id)
  const allSubjRes = await client.query('SELECT id, name FROM subjects WHERE tenant_id = $1', [TENANT_ID]);
  const subjectMap = new Map();
  allSubjRes.rows.forEach(r => subjectMap.set(r.name, r.id));

  function getSubjectId(name) {
    if (subjectMap.has(name)) return subjectMap.get(name);
    // Aliases
    if (name === 'PAI & BP' || name === 'PAI') return subjectMap.get('Pendidikan Agama Islam dan Budi Pekerti');
    if (name === 'P.PANCASILA') return subjectMap.get('Pendidikan Pancasila');
    if (name === 'MATEMATIKA') return subjectMap.get('Matematika (Umum)');
    if (name === 'BAHASA INDONESIA') return subjectMap.get('Bahasa Indonesia');
    if (name === 'IPAS') return subjectMap.get('Ilmu Pengetahuan Alam dan Sosial (IPAS)');
    if (name === 'BAHASA INGGRIS') return subjectMap.get('Bahasa Inggris');
    if (name === 'PENJASKES') return subjectMap.get('Pendidikan Jasmani, Olahraga, dan Kesehatan');
    if (name === 'SENI BUDAYA') return subjectMap.get('Seni Budaya');
    if (name === 'KETERAMPILAN KOMPUTER') return subjectMap.get('Keterampilan Komputer');
    if (name === 'IPS') return subjectMap.get('Ilmu Pengetahuan Sosial (IPS)');
    if (name === 'BAHASA CIREBON') return subjectMap.get('Bahasa Cirebon');
    if (name === 'IPA') return subjectMap.get('Ilmu Pengetahuan Alam (IPA)');
    if (name === 'SEJARAH') return subjectMap.get('Sejarah');
    if (name === 'GEOGRAFI') return subjectMap.get('Geografi');
    if (name === 'SOSIOLOGI') return subjectMap.get('Sosiologi');
    if (name === 'EKONOMI') return subjectMap.get('Ekonomi');
    if (name === 'BIOLOGI KIMIA FISIKA' || name === 'IPA ( KIMIA FISIKA BIOLOGI )') return subjectMap.get('IPA (Kimia, Fisika, Biologi)');
    throw new Error(`Subject not found: ${name}`);
  }

  // 4. Build class map
  const classesRes = await client.query('SELECT id, name FROM classes WHERE tenant_id = $1', [TENANT_ID]);
  const classMap = new Map();
  classesRes.rows.forEach(r => classMap.set(r.name, r.id));

  // 5. Clean up old test schedules
  const delRes = await client.query('DELETE FROM class_schedules WHERE tenant_id = $1', [TENANT_ID]);
  console.log(`Cleaned up ${delRes.rowCount} old schedules.`);

  // 6. Define the official schedules per Paket
  const PAKET_A_SCHEDULE = [
    // SENIN
    { day: 'Senin', start: '08:00', end: '09:30', subject: 'Pendidikan Agama Islam dan Budi Pekerti', teacher: 'EHA MEIDA KARTIKA', room: 'Kelas Online / Google Meet' },
    { day: 'Senin', start: '10:00', end: '11:30', subject: 'Pendidikan Pancasila', teacher: 'SUSILAWATI', room: 'Kelas Online / Google Meet' },
    // SELASA
    { day: 'Selasa', start: '08:00', end: '09:30', subject: 'Matematika (Umum)', teacher: 'AMELIANI DWI IRA FANISA', room: 'Kelas Online / Google Meet' },
    { day: 'Selasa', start: '10:00', end: '11:30', subject: 'Bahasa Indonesia', teacher: 'AMIN LISANA', room: 'Kelas Online / Google Meet' },
    // RABU
    { day: 'Rabu', start: '08:00', end: '09:30', subject: 'Ilmu Pengetahuan Alam dan Sosial (IPAS)', teacher: "ASEP RIFA'I", room: 'Kelas Online / Google Meet' },
    { day: 'Rabu', start: '10:00', end: '11:30', subject: 'Bahasa Inggris', teacher: 'ASYIFA R.I', room: 'Kelas Online / Google Meet' },
    // SABTU (Tatap Muka)
    { day: 'Sabtu', start: '08:00', end: '09:30', subject: 'PENJASKES', teacher: 'ASEF RIFAI', room: 'Ruang Kelas / Lapangan (Tatap Muka)' },
    { day: 'Sabtu', start: '09:45', end: '11:15', subject: 'SENI BUDAYA', teacher: 'HUSNUL CHATIM', room: 'Ruang Kelas (Tatap Muka)' },
    { day: 'Sabtu', start: '11:15', end: '12:45', subject: 'KETERAMPILAN KOMPUTER', teacher: 'IKIN BAIHAKI', room: 'Lab Komputer (Tatap Muka)' },
  ];

  const PAKET_B_SCHEDULE = [
    // SENIN
    { day: 'Senin', start: '08:00', end: '09:30', subject: 'Pendidikan Agama Islam dan Budi Pekerti', teacher: 'SRI MULYANI', room: 'Kelas Online / Google Meet' },
    { day: 'Senin', start: '10:00', end: '11:30', subject: 'Matematika (Umum)', teacher: 'KUSWANTO', room: 'Kelas Online / Google Meet' },
    // SELASA
    { day: 'Selasa', start: '08:00', end: '09:30', subject: 'Bahasa Indonesia', teacher: 'KHAERIYAH', room: 'Kelas Online / Google Meet' },
    { day: 'Selasa', start: '10:00', end: '11:30', subject: 'IPS', teacher: 'TAUFIK HIDAYAT', room: 'Kelas Online / Google Meet' },
    // RABU
    { day: 'Rabu', start: '08:00', end: '09:30', subject: 'Pendidikan Pancasila', teacher: 'KRISTIANTI', room: 'Kelas Online / Google Meet' },
    { day: 'Rabu', start: '10:00', end: '11:30', subject: 'BAHASA CIREBON', teacher: 'FITRI NAPISAH', room: 'Kelas Online / Google Meet' },
    // KAMIS
    { day: 'Kamis', start: '08:00', end: '09:30', subject: 'Bahasa Inggris', teacher: 'AS SYIFA RI', room: 'Kelas Online / Google Meet' },
    { day: 'Kamis', start: '10:00', end: '11:30', subject: 'IPA', teacher: 'ROHMANA', room: 'Kelas Online / Google Meet' },
    // SABTU (Tatap Muka)
    { day: 'Sabtu', start: '08:00', end: '09:30', subject: 'PENJASKES', teacher: 'ASEF RIFAI', room: 'Ruang Kelas / Lapangan (Tatap Muka)' },
    { day: 'Sabtu', start: '09:45', end: '11:15', subject: 'SENI BUDAYA', teacher: 'HUSNUL CHATIM', room: 'Ruang Kelas (Tatap Muka)' },
    { day: 'Sabtu', start: '11:15', end: '12:45', subject: 'KETERAMPILAN KOMPUTER', teacher: 'IKIN BAIHAKI', room: 'Lab Komputer (Tatap Muka)' },
  ];

  const PAKET_C_SCHEDULE = [
    // SENIN
    { day: 'Senin', start: '08:00', end: '09:30', subject: 'Pendidikan Agama Islam dan Budi Pekerti', teacher: 'SRI MULYANI', room: 'Kelas Online / Google Meet' },
    { day: 'Senin', start: '10:00', end: '11:30', subject: 'Matematika (Umum)', teacher: 'KUSWANTO', room: 'Kelas Online / Google Meet' },
    // SELASA
    { day: 'Selasa', start: '08:00', end: '09:30', subject: 'Bahasa Indonesia', teacher: 'KHAERIYAH', room: 'Kelas Online / Google Meet' },
    { day: 'Selasa', start: '10:00', end: '11:30', subject: 'SEJARAH', teacher: 'TAUFIK HIDAYAT', room: 'Kelas Online / Google Meet' },
    // RABU
    { day: 'Rabu', start: '08:00', end: '09:30', subject: 'Pendidikan Pancasila', teacher: 'KRISTIANTI', room: 'Kelas Online / Google Meet' },
    { day: 'Rabu', start: '10:00', end: '11:30', subject: 'GEOGRAFI', teacher: 'TAUFIK HIDAYAT', room: 'Kelas Online / Google Meet' },
    // KAMIS
    { day: 'Kamis', start: '08:00', end: '09:30', subject: 'Bahasa Inggris', teacher: 'AS SYIFA RI', room: 'Kelas Online / Google Meet' },
    { day: 'Kamis', start: '10:00', end: '11:30', subject: 'SOSIOLOGI', teacher: 'ESI ROKESI', room: 'Kelas Online / Google Meet' },
    // JUMAT
    { day: 'Jumat', start: '08:00', end: '09:30', subject: 'EKONOMI', teacher: 'FITRI NAPISAH', room: 'Kelas Online / Google Meet' },
    { day: 'Jumat', start: '09:45', end: '11:15', subject: 'BIOLOGI KIMIA FISIKA', teacher: 'ROHMANA', room: 'Kelas Online / Google Meet' },
    // SABTU (Tatap Muka)
    { day: 'Sabtu', start: '08:00', end: '09:30', subject: 'PENJASKES', teacher: 'ASEF RIFAI', room: 'Ruang Kelas / Lapangan (Tatap Muka)' },
    { day: 'Sabtu', start: '09:45', end: '11:15', subject: 'SENI BUDAYA', teacher: 'HUSNUL CHATIM', room: 'Ruang Kelas (Tatap Muka)' },
    { day: 'Sabtu', start: '11:15', end: '12:45', subject: 'KETERAMPILAN KOMPUTER', teacher: 'IKIN BAIHAKI', room: 'Lab Komputer (Tatap Muka)' },
  ];

  const classGroups = [
    { classes: ['PAKET A4', 'PAKET A5', 'PAKET A6'], schedule: PAKET_A_SCHEDULE, name: 'PAKET A' },
    { classes: ['PAKET B7', 'PAKET B8', 'PAKET B8a', 'PAKET B8b', 'PAKET B9'], schedule: PAKET_B_SCHEDULE, name: 'PAKET B' },
    { classes: ['PAKET C10', 'PAKET C11a', 'PAKET C11b', 'PAKET C12a', 'PAKET C12b'], schedule: PAKET_C_SCHEDULE, name: 'PAKET C' },
  ];

  let totalInserted = 0;

  for (const group of classGroups) {
    console.log(`\n--- Seeding Schedules for ${group.name} ---`);
    for (const className of group.classes) {
      const classId = classMap.get(className);
      if (!classId) {
        console.warn(`Class not found: ${className}, skipping...`);
        continue;
      }

      for (const item of group.schedule) {
        const subjectId = getSubjectId(item.subject);
        const teacherId = getTeacherId(item.teacher);

        await client.query(`
          INSERT INTO class_schedules (
            id, tenant_id, class_id, subject_id, teacher_id, academic_year_id,
            day_of_week, start_time, end_time, room, created_at, updated_at
          ) VALUES (
            gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW()
          )
        `, [
          TENANT_ID,
          classId,
          subjectId,
          teacherId,
          academicYearId,
          item.day,
          item.start,
          item.end,
          item.room
        ]);
        totalInserted++;
      }
      console.log(`  ✓ Inserted ${group.schedule.length} slots for ${className}`);
    }
  }

  console.log(`\n🎉 Successfully inserted a total of ${totalInserted} schedule records into database!`);

  // Verification count
  const verify = await client.query(`
    SELECT c.name as rombel, COUNT(cs.id) as total_jadwal
    FROM classes c
    JOIN class_schedules cs ON cs.class_id = c.id
    WHERE cs.deleted_at IS NULL
    GROUP BY c.name
    ORDER BY c.name ASC
  `);
  console.table(verify.rows);

  await client.end();
}

main().catch(err => {
  console.error('Fatal Error:', err);
  process.exit(1);
});
