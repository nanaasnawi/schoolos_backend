const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  console.log('--- USERS WITH FITRI ---');
  const u = await client.query("SELECT id, full_name, username, email FROM users WHERE full_name ILIKE '%fitri%' OR username ILIKE '%fitri%'");
  console.log(u.rows);

  console.log('--- TEACHERS ---');
  const t = await client.query("SELECT * FROM teachers WHERE full_name ILIKE '%fitri%' OR email ILIKE '%fitri%'");
  console.log(t.rows);

  console.log('--- SUBJECTS ---');
  const s = await client.query("SELECT id, name, code FROM subjects WHERE name ILIKE '%cirebon%' OR name ILIKE '%ekonomi%'");
  console.log(s.rows);

  console.log('--- ALL SUBJECTS ---');
  const allS = await client.query("SELECT id, name, code FROM subjects ORDER BY name");
  console.log(allS.rows);

  console.log('--- ALL CLASSES ---');
  const cls = await client.query("SELECT id, name, grade_level_id FROM classes ORDER BY name");
  console.log(cls.rows);

  await client.end();
}

run().catch(console.error);

