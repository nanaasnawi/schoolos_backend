const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const res = await client.query(`
    SELECT u.id, u.username, u.email, u.full_name, r.name as role, en.class_id, c.name as class_name
    FROM users u
    JOIN user_roles ur ON ur.user_id = u.id
    JOIN roles r ON r.id = ur.role_id
    JOIN students s ON s.user_id = u.id
    LEFT JOIN enrollments en ON en.student_id = s.id
    LEFT JOIN classes c ON c.id = en.class_id
    WHERE r.name = 'Siswa' AND c.name ILIKE '%PAKET A 4%'
    LIMIT 3
  `);
  console.log('Students:', res.rows);
  await client.end();
}

run();
