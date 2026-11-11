const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function main() {
  await client.connect();
  const cols = await client.query(`
    SELECT column_name, data_type, is_nullable
    FROM information_schema.columns
    WHERE table_name = 'academic_years'
    ORDER BY ordinal_position;
  `);
  console.log("Columns of academic_years:", cols.rows);
  await client.end();
}

main().catch(console.error);
