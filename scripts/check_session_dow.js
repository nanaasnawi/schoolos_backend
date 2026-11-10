const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const cols = await client.query(`
    SELECT column_name, data_type 
    FROM information_schema.columns 
    WHERE table_name = 'learning_sessions' AND column_name IN ('session_date', 'scheduled_at', 'start_time')
  `);
  console.log(cols.rows);

  const testDow = await client.query(`
    SELECT 
      session_date,
      session_date::text,
      EXTRACT(DOW FROM session_date) as raw_dow,
      EXTRACT(DOW FROM (session_date AT TIME ZONE 'Asia/Jakarta')) as tz_dow,
      CASE EXTRACT(DOW FROM session_date)::integer
          WHEN 0 THEN 'Minggu'
          WHEN 1 THEN 'Senin'
          WHEN 2 THEN 'Selasa'
          WHEN 3 THEN 'Rabu'
          WHEN 4 THEN 'Kamis'
          WHEN 5 THEN 'Jumat'
          WHEN 6 THEN 'Sabtu'
      END as raw_dow_name,
      CASE EXTRACT(DOW FROM (session_date AT TIME ZONE 'Asia/Jakarta'))::integer
          WHEN 0 THEN 'Minggu'
          WHEN 1 THEN 'Senin'
          WHEN 2 THEN 'Selasa'
          WHEN 3 THEN 'Rabu'
          WHEN 4 THEN 'Kamis'
          WHEN 5 THEN 'Jumat'
          WHEN 6 THEN 'Sabtu'
      END as tz_dow_name
    FROM learning_sessions
    WHERE id = 'e03dfd45-e837-439b-bc89-0f23d617abd5'
  `);
  console.log(testDow.rows);
  await client.end();
}

run();

