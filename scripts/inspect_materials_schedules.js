const fs = require('fs');
const path = require('path');
const envText = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;
const { Client } = require('pg');
const client = new Client({ connectionString: dbUrl });

(async () => {
  await client.connect();
  const mats = await client.query(`
    SELECT m.id, m.title, m.description, m.subject_id, m.teacher_id, s.name as subject_name, u.full_name as teacher_name 
    FROM learning_materials m 
    LEFT JOIN subjects s ON s.id = m.subject_id 
    LEFT JOIN teachers t ON t.id = m.teacher_id 
    LEFT JOIN users u ON u.id = t.user_id 
    WHERE m.deleted_at IS NULL
    ORDER BY m.created_at DESC
  `);
  
  for (const row of mats.rows) {
    console.log('ID:', row.id);
    console.log('Title:', row.title);
    console.log('Teacher:', row.teacher_name, '| Subject:', row.subject_name);
    if (row.teacher_id) {
      const scheds = await client.query(`
        SELECT c.name as class_name, c.id as class_id, s.name as subject_name, s.id as subject_id 
        FROM class_schedules cs 
        JOIN classes c ON c.id = cs.class_id 
        JOIN subjects s ON s.id = cs.subject_id 
        WHERE cs.teacher_id = $1
      `, [row.teacher_id]);
      console.log('Schedules:', scheds.rows.map(s => `${s.class_name} [${s.subject_name}]`).join(', '));
    }
    console.log('--------------------------------------------------');
  }
  await client.end();
})();
