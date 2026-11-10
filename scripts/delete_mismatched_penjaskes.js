const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();
  const sessionId = 'e03dfd45-e837-439b-bc89-0f23d617abd5';

  const att = await client.query('SELECT COUNT(*) FROM session_attendances WHERE session_id = $1', [sessionId]);
  console.log('Attendance count:', att.rows[0].count);

  // Delete attendance if any, then delete the session
  if (parseInt(att.rows[0].count) > 0) {
    await client.query('DELETE FROM session_attendances WHERE session_id = $1', [sessionId]);
  }
  const del = await client.query('DELETE FROM learning_sessions WHERE id = $1 RETURNING id', [sessionId]);
  console.log('Deleted session:', del.rows);

  await client.end();
}

run();

