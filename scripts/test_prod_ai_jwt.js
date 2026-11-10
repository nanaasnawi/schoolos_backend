const crypto = require('crypto');
const https = require('https');

const secret = 'f4a7c8d9e2b10a3c5e7d9b1a4f6c8e0d2b4a6c8e1d3f5a7b9c1d3e5f7a9b1c3d';

function base64Url(str) {
  return Buffer.from(str).toString('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
}

function signJwt(payload, secret) {
  const header = { alg: 'HS256', typ: 'JWT' };
  const h = base64Url(JSON.stringify(header));
  const p = base64Url(JSON.stringify(payload));
  const sig = crypto.createHmac('sha256', secret).update(`${h}.${p}`).digest('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
  return `${h}.${p}.${sig}`;
}

async function run() {
  const payload = {
    sub: '01a096e3-ce06-77f3-8f5f-1942678f0009', // student Tiyas
    tenant_id: 'b158ec8a-6ea3-468a-bed5-917e37499550',
    email: '0106310228@siswa.schoolos.id',
    full_name: 'TIYAS FEBRIYANI',
    role: 'Siswa',
    exp: Math.floor(Date.now() / 1000) + 3600 * 24
  };

  const token = signJwt(payload, secret);
  console.log('Generated JWT token');

  const postData = JSON.stringify({
    question: "Jelaskan apa itu fotosintesis secara singkat",
    subject_name: "IPA",
    grade_level: "PAKET A 4"
  });

  const options = {
    hostname: 'schoolosbackend-production.up.railway.app',
    port: 443,
    path: '/api/v1/ai/tanya-ai',
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(postData)
    }
  };

  const req = https.request(options, (res) => {
    console.log('Status Code:', res.statusCode);
    let data = '';
    res.on('data', chunk => data += chunk);
    res.on('end', () => {
      console.log('Response body:', data);
    });
  });

  req.on('error', (e) => {
    console.error('Request error:', e);
  });

  req.write(postData);
  req.end();
}

run();

