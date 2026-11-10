const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const class4 = await client.query(`
    SELECT ls.id, ls.class_id, c.name as class_name, ls.subject_id, s.name as subject_name, 
           ls.teacher_id, t.full_name as teacher_name, ls.session_date, ls.start_time, ls.end_time, 
           ls.session_type, ls.status, ls.schedule_id, cs.day_of_week as sched_dow, ls.created_at
    FROM learning_sessions ls
    JOIN classes c ON c.id = ls.class_id
    LEFT JOIN subjects s ON s.id = ls.subject_id
    LEFT JOIN teachers t ON t.id = ls.teacher_id
    LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
    WHERE c.name ILIKE '%PAKET A 4%' AND ls.session_date = '2026-11-10'
  `);
  console.log(JSON.stringify(class4.rows, null, 2));
  await client.end();
}

run();

