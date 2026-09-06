const { Client } = require('pg');
const client = new Client('postgres://school_admin:secretpassword@localhost:5433/school_os');

async function seedPermissions() {
  await client.connect();
  console.log('Connected to PostgreSQL');

  const teacherPerms = [
    'Student.Read',
    'Teacher.Read',
    'Guardian.Read',
    'Academic.Manage',
    'Learning.Curriculum.Read',
    'Learning.Syllabus.Read',
    'Learning.Material.Create', 'Learning.Material.Read', 'Learning.Material.Update', 'Learning.Material.Delete',
    'Learning.Lesson.Create', 'Learning.Lesson.Read', 'Learning.Lesson.Update', 'Learning.Lesson.Delete',
    'Learning.Session.Create', 'Learning.Session.Read', 'Learning.Session.Update', 'Learning.Session.Delete',
    'Learning.Assignment.Create', 'Learning.Assignment.Read', 'Learning.Assignment.Update', 'Learning.Assignment.Delete',
    'Learning.Quiz.Create', 'Learning.Quiz.Read', 'Learning.Quiz.Update', 'Learning.Quiz.Delete',
    'Learning.Assessment.Configure', 'Learning.Assessment.Read',
    'Learning.Progress.Read', 'Learning.Progress.Update',
    'Learning.Achievement.Create', 'Learning.Achievement.Read', 'Learning.Achievement.Award',
    'Learning.Feed.Create', 'Learning.Feed.Read',
    'Notification.Read', 'Notification.Update',
    'Assessment.Input', 'Assessment.Read',
    'Attendance.Record', 'Attendance.Read',
  ];

  const studentPerms = [
    'Student.Read',
    'Teacher.Read',
    'Learning.Curriculum.Read',
    'Learning.Syllabus.Read',
    'Learning.Material.Read',
    'Learning.Lesson.Read',
    'Learning.Session.Read',
    'Learning.Assignment.Read',
    'Learning.Quiz.Read',
    'Learning.Assessment.Read',
    'Learning.Progress.Read',
    'Learning.Achievement.Read',
    'Learning.Feed.Read',
    'Notification.Read', 'Notification.Update',
    'Assessment.Read',
    'Attendance.Read',
  ];

  const parentPerms = [
    'Student.Read',
    'Teacher.Read',
    'Learning.Session.Read',
    'Learning.Assignment.Read',
    'Learning.Progress.Read',
    'Learning.Achievement.Read',
    'Notification.Read', 'Notification.Update',
    'Assessment.Read',
    'Attendance.Read',
  ];

  // 1. Assign to Guru
  const guruRoles = await client.query("SELECT id, tenant_id FROM roles WHERE name = 'Guru'");
  for (const r of guruRoles.rows) {
    for (const p of teacherPerms) {
      await client.query('INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2) ON CONFLICT DO NOTHING', [r.id, p]);
    }
  }
  console.log(`Updated ${guruRoles.rows.length} Guru roles with ${teacherPerms.length} permissions each.`);

  // 2. Assign to Siswa
  const siswaRoles = await client.query("SELECT id, tenant_id FROM roles WHERE name = 'Siswa'");
  for (const r of siswaRoles.rows) {
    for (const p of studentPerms) {
      await client.query('INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2) ON CONFLICT DO NOTHING', [r.id, p]);
    }
  }
  console.log(`Updated ${siswaRoles.rows.length} Siswa roles with ${studentPerms.length} permissions each.`);

  // 3. Assign to Wali Siswa
  const waliRoles = await client.query("SELECT id, tenant_id FROM roles WHERE name = 'Wali Siswa'");
  for (const r of waliRoles.rows) {
    for (const p of parentPerms) {
      await client.query('INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2) ON CONFLICT DO NOTHING', [r.id, p]);
    }
  }
  console.log(`Updated ${waliRoles.rows.length} Wali Siswa roles with ${parentPerms.length} permissions each.`);

  // Verify counts
  const check = await client.query(`
    SELECT r.name, count(rp.permission) as perms 
    FROM roles r 
    JOIN role_permissions rp ON rp.role_id = r.id 
    GROUP BY r.name
  `);
  console.table(check.rows);

  await client.end();
}

seedPermissions().catch(console.error);
