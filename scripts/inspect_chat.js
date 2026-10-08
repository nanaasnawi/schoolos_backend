const { Pool } = require('pg');
const pool = new Pool({ connectionString: 'postgresql://postgres:REDACTED@REDACTED_DB_HOST:5432/railway' });
async function check() {
  const r = await pool.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'inquiry_messages' ORDER BY ordinal_position");
  console.log(r.rows);
  await pool.end();
}
check().catch(console.error);
