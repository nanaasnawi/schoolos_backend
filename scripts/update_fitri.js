const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  // 1. Update Fitri Nafisah in teachers table
  const updateRes = await client.query(`
    UPDATE teachers 
    SET subject = 'Bahasa Cirebon, Ekonomi', updated_at = NOW() 
    WHERE id = '01a096e3-cbce-7a72-86d2-aa65f1b3b273'
    RETURNING id, full_name, subject
  `);
  console.log('Updated teacher:', updateRes.rows);

  // 2. Check if user record has any subject or role field
  const uRes = await client.query(`
    SELECT id, full_name, username, email FROM users WHERE id = '01a096e3-cbca-7040-b410-4d8e19472e31'
  `);
  console.log('Fitri User:', uRes.rows);

  // 3. Check existing schedules for Fitri
  const scheds = await client.query(`
    SELECT cs.id, s.name as subject, c.name as class_name, cs.day_of_week, cs.start_time, cs.end_time
    FROM class_schedules cs
    JOIN subjects s ON cs.subject_id = s.id
    JOIN classes c ON cs.class_id = c.id
    WHERE cs.teacher_id = '01a096e3-cbce-7a72-86d2-aa65f1b3b273'
    ORDER BY s.name, c.name
  `);
  console.log('Fitri current schedules count:', scheds.rows.length);
  console.log('Fitri schedules:', scheds.rows);

  await client.end();
}

run().catch(console.error);

