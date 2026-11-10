const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const mismatches = await client.query(`
    SELECT ls.id, ls.class_id, c.name as class_name, ls.subject_id, s.name as subject_name, 
           ls.teacher_id, t.full_name as teacher_name, ls.session_date, ls.start_time, ls.end_time, 
           ls.status, ls.schedule_id, cs.day_of_week as sched_dow,
           CASE EXTRACT(DOW FROM ls.session_date)::integer
               WHEN 0 THEN 'Minggu'
               WHEN 1 THEN 'Senin'
               WHEN 2 THEN 'Selasa'
               WHEN 3 THEN 'Rabu'
               WHEN 4 THEN 'Kamis'
               WHEN 5 THEN 'Jumat'
               WHEN 6 THEN 'Sabtu'
           END as actual_dow,
           ls.created_at
    FROM learning_sessions ls
    JOIN classes c ON c.id = ls.class_id
    LEFT JOIN subjects s ON s.id = ls.subject_id
    LEFT JOIN teachers t ON t.id = ls.teacher_id
    JOIN class_schedules cs ON cs.id = ls.schedule_id
    WHERE cs.day_of_week != CASE EXTRACT(DOW FROM ls.session_date)::integer
               WHEN 0 THEN 'Minggu'
               WHEN 1 THEN 'Senin'
               WHEN 2 THEN 'Selasa'
               WHEN 3 THEN 'Rabu'
               WHEN 4 THEN 'Kamis'
               WHEN 5 THEN 'Jumat'
               WHEN 6 THEN 'Sabtu'
           END
    ORDER BY ls.created_at DESC
  `);
  console.log('Mismatched sessions count:', mismatches.rows.length);
  console.log(JSON.stringify(mismatches.rows, null, 2));
  await client.end();
}

run();

