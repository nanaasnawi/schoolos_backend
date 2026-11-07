const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  console.log('--- RECENT MATERIALS ---');
  const mats = await client.query(`
    SELECT m.id, m.title, m.class_id, c.name as class_name, m.subject_id, s.name as subject_name, m.session_id, m.created_at
    FROM learning_materials m
    LEFT JOIN classes c ON m.class_id = c.id
    LEFT JOIN subjects s ON m.subject_id = s.id
    WHERE m.title ILIKE '%Ekosistem%' OR m.title ILIKE '%Has and Have%' OR m.title ILIKE '%Recount%'
    ORDER BY m.created_at DESC
    LIMIT 20
  `);
  console.log(mats.rows);

  console.log('--- RECENT ASSIGNMENTS ---');
  const asss = await client.query(`
    SELECT a.id, a.title, a.class_id, c.name as class_name, a.subject_id, s.name as subject_name, a.session_id, a.created_at
    FROM assignments a
    LEFT JOIN classes c ON a.class_id = c.id
    LEFT JOIN subjects s ON a.subject_id = s.id
    WHERE a.title ILIKE '%Ekosistem%' OR a.title ILIKE '%Has and Have%' OR a.title ILIKE '%Recount%'
    ORDER BY a.created_at DESC
    LIMIT 20
  `);
  console.log(asss.rows);

  console.log('--- ALL MATERIALS IN LAST 48 HOURS ---');
  const recentMats = await client.query(`
    SELECT m.id, m.title, m.class_id, c.name as class_name, m.created_at
    FROM learning_materials m
    LEFT JOIN classes c ON m.class_id = c.id
    ORDER BY m.created_at DESC
    LIMIT 10
  `);
  console.log(recentMats.rows);

  console.log('--- ALL ASSIGNMENTS IN LAST 48 HOURS ---');
  const recentAss = await client.query(`
    SELECT a.id, a.title, a.class_id, c.name as class_name, a.created_at
    FROM assignments a
    LEFT JOIN classes c ON a.class_id = c.id
    ORDER BY a.created_at DESC
    LIMIT 10
  `);
  console.log(recentAss.rows);

  await client.end();
}

run().catch(console.error);

