const { Client } = require('pg');
const client = new Client('postgres://school_admin:secretpassword@localhost:5433/school_os');
client.connect().then(async () => {
  const perms = [
    'Student.Read', 'Student.Create', 'Student.Update', 'Student.Delete',
    'Teacher.Read', 'Teacher.Create', 'Teacher.Update', 'Teacher.Delete',
    'Guardian.Read', 'Guardian.Create', 'Guardian.Update', 'Guardian.Delete',
    'Staff.Read', 'Staff.Create', 'Staff.Update', 'Staff.Delete',
    'Academic.Manage',
    'Learning.Curriculum.Create', 'Learning.Curriculum.Read', 'Learning.Curriculum.Update', 'Learning.Curriculum.Delete',
    'Learning.Syllabus.Create', 'Learning.Syllabus.Read', 'Learning.Syllabus.Update', 'Learning.Syllabus.Delete',
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
    'School.Update'
  ];
  const roles = await client.query("SELECT id FROM roles WHERE name IN ('Operator/Staff', 'Kepala Sekolah')");
  for (const role of roles.rows) {
    for (const perm of perms) {
      await client.query('INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2) ON CONFLICT DO NOTHING', [role.id, perm]);
    }
  }
  console.log('Permissions granted successfully!');
  client.end();
}).catch(console.error);
