const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false,
});

async function run() {
  await client.connect();

  const tenantId = 'b158ec8a-6ea3-468a-bed5-917e37499550';
  const ainunUserId = '01a096e3-dd96-72e3-a5db-e0307e070ef2'; // Ainun in PAKET A 4

  const res = await client.query(`
    SELECT m.id, m.title, m.class_id
    FROM learning_materials m
    WHERE m.tenant_id = $1 
      AND m.deleted_at IS NULL
      AND (
          m.class_id IN (
              SELECT sibling_c.id 
              FROM students s
              JOIN enrollments en ON en.student_id = s.id
              JOIN classes en_c ON en_c.id = en.class_id
              JOIN classes sibling_c ON sibling_c.tenant_id = en_c.tenant_id 
                   AND (sibling_c.id = en_c.id OR (sibling_c.tingkat IS NOT NULL AND sibling_c.tingkat != '' AND sibling_c.tingkat = en_c.tingkat))
              WHERE s.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
          )
      )
  `, [tenantId, ainunUserId]);
  console.log('Query result count:', res.rows.length);

  await client.end();
}

run().catch(err => {
  console.error('Test error:', err);
  process.exit(1);
});
