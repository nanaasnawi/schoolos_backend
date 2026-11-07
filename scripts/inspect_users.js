const { Client } = require('pg');
const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway'
});

async function run() {
  await client.connect();
  const teachers = await client.query("SELECT id, user_id, full_name, subject, is_active FROM teachers");
  console.log('Teachers in DB:', teachers.rows);

  const uCols = await client.query("SELECT column_name FROM information_schema.columns WHERE table_name = 'users'");
  console.log('Columns in users:', uCols.rows.map(r => r.column_name).join(', '));
  const users = await client.query("SELECT * FROM users WHERE full_name ILIKE '%kepala%' OR full_name ILIKE '%muniroh%' OR full_name ILIKE '%chatim%'");
  console.log('Users in DB:', users.rows.map(u => ({ id: u.id, full_name: u.full_name, email: u.email, roles: u.roles, user_type: u.user_type })));

  await client.end();
}

run().catch(console.error);
