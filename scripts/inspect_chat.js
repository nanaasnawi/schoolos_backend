const connectionString = process.env.DATABASE_URL;
if (!connectionString) {
  console.error("FATAL: DATABASE_URL environment variable is required.");
  process.exit(1);
}
const pool = new Pool({ connectionString });
async function check() {
  const r = await pool.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'inquiry_messages' ORDER BY ordinal_position");
  console.log(r.rows);
  await pool.end();
}
check().catch(console.error);
