const { Client } = require('pg');

const client = new Client({
  connectionString: process.env.DATABASE_URL,
  ssl: false
});

async function run() {
  await client.connect();
  const cls = await client.query("SELECT id, name, grade_level_id, homeroom_teacher_id FROM classes ORDER BY name");
  console.log('Classes count:', cls.rows.length);
  console.log('Classes:', cls.rows);
  await client.end();
}

run().catch(console.error);

