const fs = require('fs');
const path = require('path');
const envText = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;
const { Client } = require('pg');
const client = new Client({ connectionString: dbUrl });

(async () => {
  await client.connect();
  const cols = await client.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'learning_materials' ORDER BY ordinal_position");
  console.table(cols.rows);
  const sample = await client.query("SELECT id, title, class_id, subject_id, teacher_id, created_by FROM learning_materials WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 15");
  console.table(sample.rows);
  await client.end();
})();
