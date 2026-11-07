const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const res = await client.query(`
    SELECT cs.id, cs.day_of_week, cs.start_time, cs.end_time, s.name as subject_name, c.name as class_name, c.id as class_id
    FROM class_schedules cs
    JOIN subjects s ON cs.subject_id = s.id
    JOIN classes c ON cs.class_id = c.id
    WHERE cs.day_of_week = 'Kamis' AND cs.start_time = '08:00'
    ORDER BY c.name
  `);
  console.log('Kamis 08:00 Schedules:', res.rows);
  await client.end();
}

run().catch(console.error);

