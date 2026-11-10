const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  const tenantId = 'b158ec8a-6ea3-468a-bed5-917e37499550';
  const studentUserId = '01a096e3-ce06-77f3-8f5f-1942678f0009'; // TIYAS FEBRIYANI
  const sessionDate = '2026-11-10';

  // Find enrolled class
  const classRes = await client.query(`
    SELECT en.class_id 
    FROM students s
    JOIN enrollments en ON en.student_id = s.id
    WHERE s.user_id = $1 AND (en.status ILIKE 'active' OR en.status IS NULL)
    LIMIT 1
  `, [studentUserId]);

  const classId = classRes.rows[0]?.class_id;
  console.log('Student classId:', classId);

  // Exact backend list query in sessions/controller.rs:
  const query = `
    SELECT 
        ls.id,
        ls.lesson_id,
        ls.session_type,
        ls.schedule_id,
        ls.subject_id,
        ls.class_id,
        ls.teacher_id,
        ls.substitute_teacher_id,
        ls.session_date,
        ls.session_number,
        ls.start_time,
        ls.end_time,
        ls.scheduled_at,
        ls.started_at,
        ls.ended_at,
        ls.status,
        c.name as class_name,
        s.name as subject_name,
        t.full_name as teacher_name,
        cs.day_of_week as sched_dow,
        cs.room
    FROM learning_sessions ls
    LEFT JOIN classes c ON c.id = ls.class_id
    LEFT JOIN subjects s ON s.id = ls.subject_id
    LEFT JOIN teachers t ON t.id = ls.teacher_id
    LEFT JOIN teachers st ON st.id = ls.substitute_teacher_id
    LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
    WHERE ls.tenant_id = $1 
      AND ls.deleted_at IS NULL
      AND ($2::uuid IS NULL OR ls.class_id = $2)
      AND ($3::date IS NULL OR ls.session_date = $3)
      AND (
          -- Ensure scheduled session strictly matches the template day_of_week
          cs.id IS NULL
          OR cs.day_of_week = CASE EXTRACT(DOW FROM ls.session_date)::integer
              WHEN 0 THEN 'Minggu'
              WHEN 1 THEN 'Senin'
              WHEN 2 THEN 'Selasa'
              WHEN 3 THEN 'Rabu'
              WHEN 4 THEN 'Kamis'
              WHEN 5 THEN 'Jumat'
              WHEN 6 THEN 'Sabtu'
          END
      )
  `;

  const res = await client.query(query, [tenantId, classId, sessionDate]);
  console.log('Result with backend day_of_week filter:', res.rows);

  await client.end();
}

run();

