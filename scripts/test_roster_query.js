const fs = require('fs');
const path = require('path');
const envText = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;
const { Client } = require('pg');
const client = new Client({ connectionString: dbUrl });

(async () => {
  await client.connect();
  const mat = (await client.query("SELECT id, title, class_id FROM learning_materials WHERE title ILIKE '%Singkatan%'")).rows[0];
  console.log('Material:', mat);
  const students = await client.query(`
    SELECT s.id, s.full_name, c.name as class_name 
    FROM students s 
    JOIN enrollments en ON en.student_id = s.id AND en.class_id = $1 
    JOIN classes c ON c.id = en.class_id 
    WHERE s.deleted_at IS NULL AND (s.status = 'active' OR s.status = 'Active' OR s.is_active = true)
  `, [mat.class_id]);
  console.log('Total students returned in roster:', students.rowCount);
  console.table(students.rows);
  await client.end();
})();
