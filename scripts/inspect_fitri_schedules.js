const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const cs = await client.query(`
    SELECT cs.id, cs.day_of_week, cs.start_time, cs.end_time, cs.room,
           s.name as subject_name, s.code as subject_code,
           c.name as class_name, c.id as class_id
    FROM class_schedules cs
    LEFT JOIN subjects s ON cs.subject_id = s.id
    LEFT JOIN classes c ON cs.class_id = c.id
    WHERE cs.teacher_id = '01a096e3-cbce-7a72-86d2-aa65f1b3b273'
    ORDER BY s.name, c.name
  `);
  console.log('Fitri Nafisah Schedules:', cs.rows);
  await client.end();
}

run().catch(console.error);

