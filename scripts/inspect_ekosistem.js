const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const m = await client.query("SELECT * FROM learning_materials WHERE title ILIKE '%Ekosistem%'");
  console.log('Materials with Ekosistem:', m.rows);
  const a = await client.query("SELECT * FROM assignments WHERE title ILIKE '%Ekosistem%'");
  console.log('Assignments with Ekosistem:', a.rows);
  await client.end();
}

run().catch(console.error);

