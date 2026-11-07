const { Client } = require('pg');
const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway'
});

async function run() {
  await client.connect();
  const q = `
    SELECT u.id, u.full_name, u.tenant_id, r.name as role_name
    FROM users u
    LEFT JOIN user_roles ur ON ur.user_id = u.id
    LEFT JOIN roles r ON r.id = ur.role_id
    WHERE u.full_name ILIKE '%kepala%' OR u.full_name ILIKE '%muniroh%' OR u.full_name ILIKE '%chatim%'
  `;
  const res = await client.query(q);
  console.log('Roles of principal users:', res.rows);

  const allRoles = await client.query("SELECT * FROM roles");
  console.log('All roles in DB:', allRoles.rows);

  await client.end();
}

run().catch(console.error);

