const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  // Let's check how many materials and assignments currently exist for PAKET B 9A vs PAKET B 9B
  const m9a = await client.query("SELECT id, title, created_at FROM learning_materials WHERE class_id = '01a10ca3-d5a7-7041-859b-5358599cf0a4' AND is_active = true");
  const m9b = await client.query("SELECT id, title, created_at FROM learning_materials WHERE class_id = '01a10ca3-d184-7a23-b116-7fd945b46f78' AND is_active = true");

  console.log('Materials in PAKET B 9A:', m9a.rows.length);
  console.log('Materials in PAKET B 9B:', m9b.rows.length);
  console.log('9A titles:', m9a.rows.map(r => r.title));
  console.log('9B titles:', m9b.rows.map(r => r.title));

  const a9a = await client.query("SELECT id, title, created_at FROM assignments WHERE class_id = '01a10ca3-d5a7-7041-859b-5358599cf0a4' AND is_active = true");
  const a9b = await client.query("SELECT id, title, created_at FROM assignments WHERE class_id = '01a10ca3-d184-7a23-b116-7fd945b46f78' AND is_active = true");

  console.log('Assignments in PAKET B 9A:', a9a.rows.length);
  console.log('Assignments in PAKET B 9B:', a9b.rows.length);
  console.log('9A ass titles:', a9a.rows.map(r => r.title));
  console.log('9B ass titles:', a9b.rows.map(r => r.title));

  // Let's also check PAKET B 8A vs PAKET B 8B
  const m8a = await client.query("SELECT id, title, created_at FROM learning_materials WHERE class_id = '01a10ca3-ce12-7c23-a3b9-e1c976370c01' AND is_active = true");
  const m8b = await client.query("SELECT id, title, created_at FROM learning_materials WHERE class_id = '01a10ca3-caa2-7861-b081-9fb19b0f3109' AND is_active = true");

  console.log('Materials in PAKET B 8A:', m8a.rows.length);
  console.log('Materials in PAKET B 8B:', m8b.rows.length);
  console.log('8A titles:', m8a.rows.map(r => r.title));
  console.log('8B titles:', m8b.rows.map(r => r.title));

  await client.end();
}

run().catch(console.error);

