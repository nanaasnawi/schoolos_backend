// READ-ONLY audit: compare GTK (teachers/staff) records vs assigned user roles.
if (!process.env.DATABASE_URL) {
  const envText = require('fs').readFileSync(require('path').join(__dirname, '..', '.env'), 'utf8');
  const m = envText.match(/^DATABASE_URL=(.+)$/m);
  if (m) process.env.DATABASE_URL = m[1].trim();
}
const { Client } = require('pg');

(async () => {
  const client = new Client({ connectionString: process.env.DATABASE_URL });
  await client.connect();
  try {
    const q = async (title, sql) => {
      const r = await client.query(sql);
      console.log(`\n=== ${title} (${r.rowCount}) ===`);
      console.table(r.rows);
    };

    await q('Roles per tenant', `SELECT t.name AS tenant, r.name, r.allowed_platforms,
      (SELECT COUNT(*) FROM user_roles ur WHERE ur.role_id = r.id) AS users
      FROM roles r JOIN tenants t ON t.id = r.tenant_id ORDER BY t.name, r.name`);

    await q('Teachers: subject/jenis_ptk vs roles', `SELECT t.full_name, t.nip, t.subject, t.jenis_ptk, t.status_kepegawaian,
      (SELECT string_agg(r.name, ', ') FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = t.user_id) AS roles
      FROM teachers t WHERE t.deleted_at IS NULL ORDER BY t.full_name`);

    await q('Staff: job_title/jenis_ptk vs roles', `SELECT s.full_name, s.nip, s.job_title, s.jenis_ptk,
      (SELECT string_agg(r.name, ', ') FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = s.user_id) AS roles
      FROM staff s WHERE s.deleted_at IS NULL ORDER BY s.full_name`);

    await q('People present in BOTH teachers and staff', `SELECT t.full_name FROM teachers t
      JOIN staff s ON s.tenant_id = t.tenant_id AND UPPER(s.full_name) = UPPER(t.full_name)
      WHERE t.deleted_at IS NULL AND s.deleted_at IS NULL`);

    await q('Users with @guru.schoolos.id email & their roles', `SELECT u.full_name, u.email,
      (SELECT string_agg(r.name, ', ') FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = u.id) AS roles,
      EXISTS(SELECT 1 FROM teachers t WHERE t.user_id = u.id AND t.deleted_at IS NULL) AS in_teachers,
      EXISTS(SELECT 1 FROM staff s WHERE s.user_id = u.id AND s.deleted_at IS NULL) AS in_staff
      FROM users u WHERE u.email ILIKE '%@guru.schoolos.id' ORDER BY u.full_name`);
  } finally {
    await client.end();
  }
})().catch((e) => { console.error(e); process.exit(1); });
