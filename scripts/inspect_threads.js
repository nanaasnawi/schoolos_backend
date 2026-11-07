const { Client } = require('pg');
const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway'
});

async function run() {
  await client.connect();
  const cols = await client.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'inquiry_threads'");
  console.log('Columns in inquiry_threads:', cols.rows.map(r => r.column_name).join(', '));
  
  const msgCols = await client.query("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'inquiry_messages'");
  console.log('Columns in inquiry_messages:', msgCols.rows.map(r => r.column_name).join(', '));

  const threads = await client.query("SELECT * FROM inquiry_threads ORDER BY updated_at DESC LIMIT 5");
  console.log('Recent 5 threads:');
  for (const t of threads.rows) {
    console.log(`- Thread ID: ${t.id}, student_id: ${t.student_id}, student_name: ${t.student_name}, teacher_id: ${t.teacher_id}, teacher_name: ${t.teacher_name}, subject: ${t.subject_name}, ref_title: ${t.reference_title}, updated_at: ${t.updated_at}`);
  }

  const msgs = await client.query("SELECT * FROM inquiry_messages ORDER BY created_at DESC LIMIT 5");
  console.log('Recent 5 messages:');
  for (const m of msgs.rows) {
    console.log(`- Msg ID: ${m.id}, thread_id: ${m.thread_id}, sender_id: ${m.sender_id}, sender_name: ${m.sender_name}, sender_role: ${m.sender_role}, content: ${m.content}, created_at: ${m.created_at}`);
  }

  await client.end();
}

run().catch(console.error);

