const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  console.log('--- COLUMNS IN TEACHERS ---');
  const cols = await client.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'teachers'");
  console.log(cols.rows);

  console.log('--- TABLES WITH TEACHER OR SCHEDULE ---');
  const tbls = await client.query("SELECT table_name FROM information_schema.tables WHERE table_schema='public' AND (table_name ILIKE '%teacher%' OR table_name ILIKE '%schedule%' OR table_name ILIKE '%subject%' OR table_name ILIKE '%session%')");
  console.log(tbls.rows);

  console.log('--- CLASS_SCHEDULES FOR FITRI ---');
  const cs = await client.query("SELECT * FROM class_schedules WHERE teacher_id = '01a096e3-cbce-7a72-86d2-aa65f1b3b273'");
  console.log(cs.rows);

  console.log('--- SAMPLE CLASS_SCHEDULES ---');
  const sampleCs = await client.query("SELECT cs.*, s.name as subject_name, c.name as class_name FROM class_schedules cs LEFT JOIN subjects s ON cs.subject_id = s.id LEFT JOIN classes c ON cs.class_id = c.id LIMIT 10");
  console.log(sampleCs.rows);

  await client.end();
}

run().catch(console.error);

