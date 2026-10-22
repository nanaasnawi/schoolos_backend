const { Client } = require('pg');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const connectionString = process.env.DATABASE_URL;
if (!connectionString) {
  console.error("FATAL: DATABASE_URL environment variable is required.");
  process.exit(1);
}

const client = new Client({
  connectionString,
  ssl: false,
});

async function run() {
  await client.connect();
  console.log('Connected to DB');

  const pending = [
    '20260918131500_delete_non_sibi_and_broken_library_books.sql',
    '20260926000000_relax_session_attendance_fkey.sql',
    '20260926010000_seed_assignment_questions_and_submissions.sql',
    '20260930120000_enhance_attendance_system.sql',
    '20260930140000_enhance_assignments_system.sql',
    '20260930150000_add_assignment_submit_permission.sql',
    '20261004080000_add_avatar_url_to_users.sql',
    '20261008020000_create_student_xp_gamification.sql',
    '20261008040000_add_is_active_to_students.sql',
    '20261008050000_add_avatar_binary_to_users.sql',
    '20261009120000_timetable_hub_core_schema.sql',
    '20261009130000_scope_classes_grade_levels.sql',
    '20261011200000_fix_quiz_attempts_multi_attempt_unique.sql',
    '20261012183000_create_kosp_ecosystem.sql',
    '20261014030000_standardize_four_attendance_statuses.sql'
  ];

  const migrationsDir = path.join(__dirname, '..', 'migrations');

  for (const file of pending) {
    const filePath = path.join(migrationsDir, file);
    if (!fs.existsSync(filePath)) {
      console.log(`File not found: ${file}`);
      continue;
    }
    const match = file.match(/^(\d+)_(.+)\.sql$/);
    const version = match[1];
    const description = match[2].replace(/_/g, ' ');
    const sql = fs.readFileSync(filePath, 'utf8');

    console.log(`\nApplying: ${file}...`);
    const startTime = Date.now();
    try {
      await client.query(sql);
      const executionTime = (Date.now() - startTime) * 1000000; // in nanoseconds
      const checksum = crypto.createHash('sha384').update(sql).digest();

      await client.query(`
        INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
        VALUES ($1, $2, NOW(), true, $3, $4)
        ON CONFLICT (version) DO UPDATE SET success = true
      `, [version, description, checksum, executionTime.toString()]);

      console.log(`✓ SUCCESS: ${file}`);
    } catch (err) {
      console.error(`✗ FAILED: ${file}`, err.message);
      // We continue to see other errors or fix them
    }
  }

  // Verify columns on session_attendances
  const cols = await client.query(`
    SELECT column_name, data_type 
    FROM information_schema.columns 
    WHERE table_name = 'session_attendances';
  `);
  console.log('\nFinal columns in session_attendances:');
  console.log(cols.rows.map(c => `${c.column_name} (${c.data_type})`));

  await client.end();
}

run().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
