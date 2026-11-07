const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const res = await client.query(`
    SELECT a.id, a.title, a.class_id, c.name as class_name, a.subject_id, s.name as subject_name
    FROM assignments a
    LEFT JOIN classes c ON a.class_id = c.id
    LEFT JOIN subjects s ON a.subject_id = s.id
    WHERE a.is_active = true
    ORDER BY a.created_at DESC
    LIMIT 10
  `);
  console.log('Assignments in DB:', res.rows);
  await client.end();
}

run().catch(console.error);

