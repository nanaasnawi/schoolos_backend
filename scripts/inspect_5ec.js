const { Client } = require('pg');
const client = new Client({
  connectionString: process.env.DATABASE_URL
});

async function run() {
  await client.connect();
  const res = await client.query("SELECT * FROM inquiry_threads WHERE id = '5ec494a8-cf63-4b4d-beec-e397ff1a7b37'");
  console.log('Thread 5ec494a8:', res.rows[0]);

  const msgs = await client.query("SELECT * FROM inquiry_messages WHERE thread_id = '5ec494a8-cf63-4b4d-beec-e397ff1a7b37' ORDER BY created_at ASC");
  console.log('Messages in 5ec494a8:', msgs.rows);

  await client.end();
}

run().catch(console.error);

