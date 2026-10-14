const { Client } = require('pg');
const fs = require('fs');
const path = require('path');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false,
});

async function run() {
  await client.connect();
  console.log('Connected to DB');

  const migs = await client.query(`SELECT version FROM _sqlx_migrations`);
  const appliedVersions = new Set(migs.rows.map(r => r.version.toString()));

  const migrationsDir = path.join(__dirname, '..', 'migrations');
  const files = fs.readdirSync(migrationsDir)
    .filter(f => f.endsWith('.sql'))
    .sort();

  console.log(`Total migration files found: ${files.length}`);
  const missing = [];

  for (const f of files) {
    // Extract version prefix (e.g. 20260930120000 or 0001)
    const match = f.match(/^(\d+)_(.+)\.sql$/);
    if (!match) continue;
    const version = match[1];
    // In sqlx, 0001 is stored as 1
    const normalized = version.replace(/^0+/, '');
    if (!appliedVersions.has(version) && !appliedVersions.has(normalized)) {
      missing.push({ file: f, version });
    }
  }

  console.log('Missing migrations count:', missing.length);
  for (const m of missing) {
    console.log(`- ${m.file} (version: ${m.version})`);
  }

  await client.end();
}

run().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});
