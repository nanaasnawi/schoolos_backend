const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgres://school_admin:secretpassword@localhost:5433/school_os'
});

async function main() {
  await client.connect();

  console.log('=== SUMMARY PER GURU PENGAMPU ===');
  const teacherRes = await client.query(`
    SELECT t.full_name as teacher, COUNT(cs.id) as total_jadwal, array_agg(DISTINCT s.name) as subjects
    FROM teachers t
    JOIN class_schedules cs ON cs.teacher_id = t.id
    JOIN subjects s ON s.id = cs.subject_id
    WHERE cs.deleted_at IS NULL
    GROUP BY t.full_name
    ORDER BY t.full_name ASC
  `);
  console.table(teacherRes.rows.map(r => ({
    teacher: r.teacher,
    total_jadwal: r.total_jadwal,
    subjects: r.subjects.join(', ')
  })));

  console.log('\n=== SAMPLE JADWAL: PAKET A4 ===');
  const a4 = await client.query(`
    SELECT cs.day_of_week, cs.start_time || ' - ' || cs.end_time as jam, s.name as mapel, t.full_name as guru, cs.room
    FROM class_schedules cs
    JOIN classes c ON c.id = cs.class_id
    JOIN subjects s ON s.id = cs.subject_id
    JOIN teachers t ON t.id = cs.teacher_id
    WHERE c.name = 'PAKET A4' AND cs.deleted_at IS NULL
    ORDER BY 
      CASE cs.day_of_week
        WHEN 'Senin' THEN 1
        WHEN 'Selasa' THEN 2
        WHEN 'Rabu' THEN 3
        WHEN 'Kamis' THEN 4
        WHEN 'Jumat' THEN 5
        WHEN 'Sabtu' THEN 6
        ELSE 7
      END, cs.start_time
  `);
  console.table(a4.rows);

  console.log('\n=== SAMPLE JADWAL: PAKET B8a ===');
  const b8a = await client.query(`
    SELECT cs.day_of_week, cs.start_time || ' - ' || cs.end_time as jam, s.name as mapel, t.full_name as guru, cs.room
    FROM class_schedules cs
    JOIN classes c ON c.id = cs.class_id
    JOIN subjects s ON s.id = cs.subject_id
    JOIN teachers t ON t.id = cs.teacher_id
    WHERE c.name = 'PAKET B8a' AND cs.deleted_at IS NULL
    ORDER BY 
      CASE cs.day_of_week
        WHEN 'Senin' THEN 1
        WHEN 'Selasa' THEN 2
        WHEN 'Rabu' THEN 3
        WHEN 'Kamis' THEN 4
        WHEN 'Jumat' THEN 5
        WHEN 'Sabtu' THEN 6
        ELSE 7
      END, cs.start_time
  `);
  console.table(b8a.rows);

  console.log('\n=== SAMPLE JADWAL: PAKET C11a ===');
  const c11a = await client.query(`
    SELECT cs.day_of_week, cs.start_time || ' - ' || cs.end_time as jam, s.name as mapel, t.full_name as guru, cs.room
    FROM class_schedules cs
    JOIN classes c ON c.id = cs.class_id
    JOIN subjects s ON s.id = cs.subject_id
    JOIN teachers t ON t.id = cs.teacher_id
    WHERE c.name = 'PAKET C11a' AND cs.deleted_at IS NULL
    ORDER BY 
      CASE cs.day_of_week
        WHEN 'Senin' THEN 1
        WHEN 'Selasa' THEN 2
        WHEN 'Rabu' THEN 3
        WHEN 'Kamis' THEN 4
        WHEN 'Jumat' THEN 5
        WHEN 'Sabtu' THEN 6
        ELSE 7
      END, cs.start_time
  `);
  console.table(c11a.rows);

  await client.end();
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
