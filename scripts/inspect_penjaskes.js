const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function check() {
  await client.connect();

  const sessions = await client.query(`
    SELECT ls.*, c.name as class_name, s.name as subject_name, 
           t.full_name as teacher_name, cs.day_of_week as sched_dow
    FROM learning_sessions ls
    JOIN classes c ON c.id = ls.class_id
    LEFT JOIN subjects s ON s.id = ls.subject_id
    LEFT JOIN teachers t ON t.id = ls.teacher_id
    LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
    WHERE s.name ILIKE '%Jasmani%' OR t.full_name ILIKE '%ASEP RIFAI%'
    ORDER BY ls.session_date DESC
    LIMIT 20
  `);
  console.log('--- SESSIONS FOR PENJASKES / ASEP RIFAI ---');
  console.log(JSON.stringify(sessions.rows, null, 2));

  const schedules = await client.query(`
    SELECT cs.*, c.name as class_name, s.name as subject_name,
           t.full_name as teacher_name
    FROM class_schedules cs
    JOIN classes c ON c.id = cs.class_id
    LEFT JOIN subjects s ON s.id = cs.subject_id
    LEFT JOIN teachers t ON t.id = cs.teacher_id
    WHERE s.name ILIKE '%Jasmani%' OR t.full_name ILIKE '%ASEP RIFAI%'
  `);
  console.log('--- SCHEDULES FOR PENJASKES / ASEP RIFAI ---');
  console.log(JSON.stringify(schedules.rows, null, 2));

  // Also check all sessions on 2026-11-10 for PAKET A 4
  const class4 = await client.query(`
    SELECT ls.*, c.name as class_name, s.name as subject_name, 
           t.full_name as teacher_name, cs.day_of_week as sched_dow
    FROM learning_sessions ls
    JOIN classes c ON c.id = ls.class_id
    LEFT JOIN subjects s ON s.id = ls.subject_id
    LEFT JOIN teachers t ON t.id = ls.teacher_id
    LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
    WHERE c.name ILIKE '%PAKET A 4%' AND ls.session_date = '2026-11-10'
  `);
  console.log('--- ALL SESSIONS ON 2026-11-10 FOR PAKET A 4 ---');
  console.log(JSON.stringify(class4.rows, null, 2));

  // And check class_schedules for PAKET A 4
  const class4Scheds = await client.query(`
    SELECT cs.*, c.name as class_name, s.name as subject_name,
           t.full_name as teacher_name
    FROM class_schedules cs
    JOIN classes c ON c.id = cs.class_id
    LEFT JOIN subjects s ON s.id = cs.subject_id
    LEFT JOIN teachers t ON t.id = cs.teacher_id
    WHERE c.name ILIKE '%PAKET A 4%'
    ORDER BY cs.day_of_week, cs.start_time
  `);
  console.log('--- ALL SCHEDULES FOR PAKET A 4 ---');
  console.log(JSON.stringify(class4Scheds.rows, null, 2));

  await client.end();
}

check().catch(e => { console.error(e); client.end(); });
