const { Client } = require('pg');

async function auditSubjects() {
  const client = new Client('postgres://school_admin:secretpassword@localhost:5433/school_os');
  await client.connect();

  const cols = await client.query(`
    SELECT column_name, data_type 
    FROM information_schema.columns 
    WHERE table_name = 'subjects'
  `);
  console.log('Columns in subjects:');
  cols.rows.forEach(r => console.log(`  - ${r.column_name}: ${r.data_type}`));

  const rows = await client.query(`
    SELECT id, tenant_id, code, name 
    FROM subjects 
    ORDER BY name 
    LIMIT 25
  `);
  console.log('\nSample subjects in DB:');
  console.table(rows.rows);

  await client.end();
}

auditSubjects().catch(console.error);
