// generate_demo_qrs.js
// Script to generate QR Badge tokens for sample users and save PNG files in scratch/qrcodes

const { Client } = require('pg');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const outDir = path.join(__dirname, '..', '..', 'scratch', 'qrcodes');
if (!fs.existsSync(outDir)) {
  fs.mkdirSync(outDir, { recursive: true });
}


const client = new Client({
  connectionString: 'postgres://school_admin:secretpassword@localhost:5433/school_os'
});

async function main() {
  await client.connect();
  console.log('Connected to PostgreSQL database...');

  // Let's pick 3 sample users: 1 Siswa, 1 Guru, 1 Admin
  const res = await client.query(`
    SELECT u.id, u.tenant_id, u.email, u.full_name, COALESCE(r.name, 'Siswa') as role_name
    FROM users u
    LEFT JOIN user_roles ur ON u.id = ur.user_id
    LEFT JOIN roles r ON ur.role_id = r.id
    WHERE u.is_active = true
    ORDER BY u.created_at ASC
    LIMIT 6
  `);

  console.log(`Found ${res.rows.length} users to generate QR Badges for.`);

  let QRCode;
  try {
    QRCode = require('qrcode');
  } catch (e) {
    console.log('qrcode package not found yet, will write raw text files and SVG.');
  }

  const generatedList = [];

  for (const user of res.rows) {
    const tokenId = crypto.randomUUID();
    const entropy = crypto.randomBytes(12).toString('hex');
    const rawToken = `sch_qr_v1_${tokenId.replace(/-/g, '')}_${entropy}`;
    
    // Compute SHA-256 hash
    const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex');

    // Insert or replace in user_qr_tokens
    await client.query(`
      INSERT INTO user_qr_tokens (
        id, tenant_id, user_id, token_hash, token_type, label, is_active, created_at, updated_at
      ) VALUES (
        $1, $2, $3, $4, 'BADGE', $5, true, NOW(), NOW()
      )
    `, [tokenId, user.tenant_id, user.id, tokenHash, `Kartu Digital - ${user.full_name}`]);

    const sanitizedName = user.full_name.replace(/[^a-zA-Z0-9]/g, '_').toLowerCase();
    const txtPath = path.join(outDir, `${sanitizedName}_${user.role_name}.txt`);
    fs.writeFileSync(txtPath, rawToken, 'utf8');

    if (QRCode) {
      const pngPath = path.join(outDir, `${sanitizedName}_${user.role_name}.png`);
      await QRCode.toFile(pngPath, rawToken, {
        width: 400,
        margin: 2,
        color: {
          dark: '#0B0F19',
          light: '#FFFFFF'
        }
      });
      console.log(`✅ Generated PNG QR: ${pngPath}`);
    }

    generatedList.push({
      full_name: user.full_name,
      email: user.email,
      role: user.role_name,
      raw_token: rawToken,
    });
  }

  console.log('\n--- DAFTAR QR BADGE TOKEN GENERATED ---');
  console.log(JSON.stringify(generatedList, null, 2));

  await client.end();
}

main().catch(err => {
  console.error('Error generating demo QRs:', err);
  process.exit(1);
});
