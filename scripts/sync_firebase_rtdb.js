const { Pool } = require('pg');

const pool = new Pool({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway'
});

async function syncFirebaseToDb() {
  console.log('Fetching inquiries from Firebase RTDB...');
  const res = await fetch('https://akselerasi-edu-default-rtdb.asia-southeast1.firebasedatabase.app/inquiries.json');
  const data = await res.json();
  if (!data) {
    console.log('No data found in Firebase RTDB');
    return;
  }

  for (const [threadId, threadData] of Object.entries(data)) {
    console.log(`Processing thread: ${threadId}`);
    
    // Check if thread exists in DB
    const threadRes = await pool.query('SELECT id, tenant_id FROM inquiry_threads WHERE id = $1', [threadId]);
    if (threadRes.rows.length === 0) {
      console.log(`Thread ${threadId} not found in DB, skipping`);
      continue;
    }
    const tenantId = threadRes.rows[0].tenant_id;

    const messages = threadData.messages || {};
    const sortedMsgs = Object.values(messages).sort((a, b) => (a.timestamp || 0) - (b.timestamp || 0));

    for (const msg of sortedMsgs) {
      // Check if message already exists by content and thread_id
      const exists = await pool.query(
        'SELECT id FROM inquiry_messages WHERE thread_id = $1 AND content = $2 LIMIT 1',
        [threadId, msg.content]
      );
      if (exists.rows.length > 0) {
        continue;
      }

      const clientUuidStr = msg.id.replace(/^temp-/, '');
      const clientUuid = clientUuidStr.length === 36 ? clientUuidStr : null;
      const createdAt = msg.timestamp ? new Date(msg.timestamp) : new Date();

      await pool.query(
        `INSERT INTO inquiry_messages 
         (id, tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, client_message_id, created_at)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9)`,
        [
          tenantId,
          threadId,
          msg.senderId || 'sender',
          msg.senderName || 'User',
          msg.senderRole || (msg.isFromTeacher ? 'TEACHER' : 'STUDENT'),
          msg.content,
          !!msg.isFromTeacher,
          clientUuid,
          createdAt
        ]
      );
      console.log(`Inserted message: [${msg.senderRole}] ${msg.senderName}: "${msg.content}"`);
    }

    // Determine latest message & status
    const lastMsg = sortedMsgs[sortedMsgs.length - 1];
    if (lastMsg) {
      const isAnswered = !!lastMsg.isFromTeacher;
      const newStatus = isAnswered ? 'ANSWERED' : 'WAITING_REPLY';
      const lastMsgDate = lastMsg.timestamp ? new Date(lastMsg.timestamp) : new Date();

      await pool.query(
        `UPDATE inquiry_threads
         SET status = $1, last_message_content = $2, last_message_at = $3, updated_at = NOW()
         WHERE id = $4`,
        [newStatus, lastMsg.content, lastMsgDate, threadId]
      );
      console.log(`Updated thread ${threadId} status to: ${newStatus}, last message: "${lastMsg.content}"`);
    }
  }

  await pool.end();
  console.log('Sync complete!');
}

syncFirebaseToDb().catch(console.error);
