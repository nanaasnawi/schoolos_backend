const { Client } = require('pg');
(async () => {
  const c = new Client(process.env.DATABASE_URL || 'postgres://school_admin@localhost:5433/school_os');
  await c.connect();
  const t = await c.query(`SELECT id, full_name, subject, user_id FROM teachers WHERE full_name ILIKE '%HUSNUL%' OR full_name ILIKE '%FITRI N%'`);
  console.table(t.rows);
  const s = await c.query(`SELECT id, name FROM subjects WHERE name ILIKE '%sejarah%' OR name ILIKE '%seni%'`);
  console.table(s.rows);
  await c.end();
})().catch(e => { console.error(e.message); process.exit(1); });

