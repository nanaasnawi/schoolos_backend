const { Client } = require('pg');
const crypto = require('crypto');

async function main() {
  const client = new Client({
    connectionString: 'postgresql://school_admin:secretpassword@127.0.0.1:5433/school_os',
  });

  await client.connect();
  console.log('Connected to database.');

  try {
    // 1. Get all guardians
    const guardiansRes = await client.query(`
      SELECT id, tenant_id, full_name, phone_number 
      FROM guardians
    `);

    console.log(`Processing ${guardiansRes.rows.length} guardians.`);

    // 2. Fetch role for Wali Siswa
    const roleRes = await client.query(`
      SELECT id, tenant_id FROM roles WHERE name = 'Wali Siswa'
    `);
    const roleByTenant = {};
    for (const r of roleRes.rows) {
      roleByTenant[r.tenant_id] = r.id;
    }

    const defaultPasswordHash = '$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY';

    let successCount = 0;
    for (const g of guardiansRes.rows) {
      let roleId = roleByTenant[g.tenant_id];
      if (!roleId) {
        const newRoleId = crypto.randomUUID();
        await client.query(`
          INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
          VALUES ($1, $2, 'Wali Siswa', 'Orang Tua / Wali Siswa', 'ANDROID', true, NOW(), NOW())
          ON CONFLICT DO NOTHING
        `, [newRoleId, g.tenant_id]);
        roleId = newRoleId;
        roleByTenant[g.tenant_id] = newRoleId;
      }

      const userId = crypto.randomUUID();
      // Ensure unique email using guardian's full UUID
      const email = `wali-${g.id}@wali.schoolos.id`;

      // Insert or get user
      const userRes = await client.query(`
        INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, true, NOW(), NOW())
        ON CONFLICT (tenant_id, email) DO UPDATE SET full_name = EXCLUDED.full_name, updated_at = NOW()
        RETURNING id
      `, [userId, g.tenant_id, email, defaultPasswordHash, g.full_name]);

      const finalUserId = userRes.rows[0].id;

      // Assign user_role
      await client.query(`
        INSERT INTO user_roles (user_id, role_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
      `, [finalUserId, roleId]);

      // Link guardian.user_id
      await client.query(`
        UPDATE guardians
        SET user_id = $1, updated_at = NOW()
        WHERE id = $2
      `, [finalUserId, g.id]);

      successCount++;
    }

    console.log(`Successfully provisioned individual accounts for all ${successCount} guardians with role 'Wali Siswa'!`);
  } catch (err) {
    console.error('Error during guardian provisioning:', err);
  } finally {
    await client.end();
  }
}

main();
