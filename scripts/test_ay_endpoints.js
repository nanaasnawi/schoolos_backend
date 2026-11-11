const crypto = require('crypto');

function createJwt(payload, secret) {
  const header = { alg: 'HS256', typ: 'JWT' };
  const encode = obj => Buffer.from(JSON.stringify(obj)).toString('base64url');
  const unsignedToken = `${encode(header)}.${encode(payload)}`;
  const signature = crypto.createHmac('sha256', secret).update(unsignedToken).digest('base64url');
  return `${unsignedToken}.${signature}`;
}

async function testApi() {
  const secret = 'f4a7c8d9e2b10a3c5e7d9b1a4f6c8e0d2b4a6c8e1d3f5a7b9c1d3e5f7a9b1c3d';
  const token = createJwt({
    sub: '01a092fc-af59-76b2-85f2-cca8b8d8d515',
    tenant_id: 'b158ec8a-6ea3-468a-bed5-917e37499550',
    role: 'Principal',
    roles: ['Kepala Sekolah', 'Principal', 'Admin'],
    exp: Math.floor(Date.now() / 1000) + 3600
  }, secret);

  const endpoints = [
    '/api/v1/academic/years',
    '/api/v1/academic/academic-years',
    '/api/v1/academic-years',
  ];

  for (const ep of endpoints) {
    const res = await fetch(`https://schoolosbackend-production.up.railway.app${ep}`, {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    console.log(`Endpoint ${ep} -> status ${res.status}`);
    const text = await res.text();
    console.log(`Body preview: ${text.slice(0, 300)}`);
  }
}

testApi().catch(console.error);
