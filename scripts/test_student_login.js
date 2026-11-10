const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const u = await client.query(`SELECT id, username, password_hash FROM users WHERE username = 'tiyas.febriyani'`);
  console.log('User:', u.rows[0]);

  // Let's do a curl to login or check sessions
  await client.end();
}

run();

