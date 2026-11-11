const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function main() {
  await client.connect();
  const rows = await client.query("SELECT * FROM academic_years;");
  console.log("Academic years in DB count:", rows.rows.length);
  console.log("Academic years in DB:", rows.rows);
  await client.end();
}

main().catch(console.error);
