const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

const crypto = require('crypto');

function createJwt(payload, secret) {
  const header = { alg: 'HS256', typ: 'JWT' };
  const encode = obj => Buffer.from(JSON.stringify(obj)).toString('base64url');
  const unsignedToken = `${encode(header)}.${encode(payload)}`;
  const signature = crypto.createHmac('sha256', secret).update(unsignedToken).digest('base64url');
  return `${unsignedToken}.${signature}`;
}

async function testLivePatch() {
  const secret = 'f4a7c8d9e2b10a3c5e7d9b1a4f6c8e0d2b4a6c8e1d3f5a7b9c1d3e5f7a9b1c3d';
  const token = createJwt({
    sub: '01a092fc-af59-76b2-85f2-cca8b8d8d515',
    tenant_id: 'b158ec8a-6ea3-468a-bed5-917e37499550',
    role: 'Principal',
    exp: Math.floor(Date.now() / 1000) + 3600
  }, secret);

  const payload = {
    title: 'Tugas',
    instructions: undefined,
    max_score: 20,
    // due_at omitted because it is in the past or unchanged
    release_at: undefined,
    class_id: '01a10ca3-caa2-7861-b081-9fb19b0f3109',
    subject_id: '01a096c4-89ce-78a0-97eb-d5d85196395b',
    teacher_id: '01a09459-0096-7c93-9c86-90c7e008a0d2'
  };

  console.log('Sending PATCH to Railway...');
  const res = await fetch('https://schoolosbackend-production.up.railway.app/api/v1/learning/assignments/01a1bb48-4d16-7881-8bcb-8904eb61b6d7', {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`
    },
    body: JSON.stringify(payload)
  });

  console.log('Status:', res.status);
  const text = await res.text();
  console.log('Body:', text);
}

testLivePatch().catch(console.error);

