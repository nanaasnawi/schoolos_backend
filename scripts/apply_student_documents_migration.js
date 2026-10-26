const fs = require('fs');
const path = require('path');
const { Client } = require('pg');

const envPath = path.join(__dirname, '..', '.env');
const envText = fs.readFileSync(envPath, 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;

const client = new Client({ connectionString: dbUrl });

(async () => {
  try {
    await client.connect();
    console.log('DB Connected successfully!');
    const migrationFile = path.join(__dirname, '..', 'migrations', '20261026000000_create_student_documents_vault.sql');
    const sql = fs.readFileSync(migrationFile, 'utf8');
    await client.query(sql);
    console.log('Migration 20261026000000_create_student_documents_vault.sql applied successfully!');
    
    const res = await client.query("SELECT column_name, data_type, is_nullable FROM information_schema.columns WHERE table_name = 'student_documents' ORDER BY ordinal_position");
    console.log('Columns in student_documents:');
    console.table(res.rows);
  } catch (err) {
    console.error('Error applying migration:', err.message);
    process.exit(1);
  } finally {
    await client.end();
  }
})();
