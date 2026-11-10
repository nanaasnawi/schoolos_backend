const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  const query = `
    DELETE FROM learning_sessions ls
    USING class_schedules cs
    WHERE cs.id = ls.schedule_id
      AND cs.day_of_week != CASE EXTRACT(DOW FROM ls.session_date)::integer
          WHEN 0 THEN 'Minggu'
          WHEN 1 THEN 'Senin'
          WHEN 2 THEN 'Selasa'
          WHEN 3 THEN 'Rabu'
          WHEN 4 THEN 'Kamis'
          WHEN 5 THEN 'Jumat'
          WHEN 6 THEN 'Sabtu'
      END
      AND NOT EXISTS (SELECT 1 FROM session_attendances sa WHERE sa.session_id = ls.id)
    RETURNING ls.id, ls.session_date, cs.day_of_week;
  `;

  const res = await client.query(query);
  console.log('Cleaned up mismatched sessions count:', res.rows.length);
  console.log(res.rows);

  await client.end();
}

run();

