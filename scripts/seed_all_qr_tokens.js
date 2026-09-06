const { Client } = require('pg');
const crypto = require('crypto');

const client = new Client({
  connectionString: 'postgres://school_admin:secretpassword@localhost:5433/school_os'
});

async function main() {
  await client.connect();
  console.log('Connected to PostgreSQL database...');

  // Find all active users who do NOT have an active QR token
  const res = await client.query(`
    SELECT u.id, u.tenant_id, u.email, u.full_name, COALESCE(r.name, 'Siswa') as role_name
    FROM users u
    LEFT JOIN user_roles ur ON u.id = ur.user_id
    LEFT JOIN roles r ON ur.role_id = r.id
    WHERE u.is_active = true
      AND NOT EXISTS (
        SELECT 1 FROM user_qr_tokens q WHERE q.user_id = u.id AND q.is_active = true
      )
  `);

  console.log(`Found ${res.rows.length} users missing active QR tokens.`);

  let insertedCount = 0;
  for (const user of res.rows) {
    const tokenId = crypto.randomUUID();
    const entropy = crypto.randomBytes(12).toString('hex');
    const rawToken = `sch_qr_v1_${tokenId.replace(/-/g, '')}_${entropy}`;
    const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex');

    await client.query(`
      INSERT INTO user_qr_tokens (
        id, tenant_id, user_id, token_hash, token_type, label, is_active, created_at, updated_at
      ) VALUES (
        $1, $2, $3, $4, 'BADGE', $5, true, NOW(), NOW()
      )
    `, [tokenId, user.tenant_id, user.id, tokenHash, `Kartu Digital - ${user.full_name}`]);

    insertedCount++;
  }

  console.log(`Successfully generated and activated QR tokens for ${insertedCount} users!`);
  await client.end();
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
