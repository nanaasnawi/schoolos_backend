const http = require('http');

function post(path, body, headers = {}) {
  return new Promise((resolve, reject) => {
    const data = JSON.stringify(body);
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port: 8000,
        path,
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(data),
          ...headers,
        },
      },
      (res) => {
        let respData = '';
        res.on('data', (chunk) => (respData += chunk));
        res.on('end', () => {
          try {
            resolve({ status: res.statusCode, body: JSON.parse(respData) });
          } catch (e) {
            resolve({ status: res.statusCode, body: respData });
          }
        });
      }
    );
    req.on('error', reject);
    req.write(data);
    req.end();
  });
}

async function testQrLogin() {
  const adminToken = 'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIwMWEwMTY5Mi05MTVkLTdlMDAtYjNmMC1mNGFiNzc5YWZiYWEiLCJ0ZW5hbnRfaWQiOiJlM2M0ZDhkYS1mZjQ0LTRkODctYmIxMy03NTlhNTRmNDliZDAiLCJlbWFpbCI6ImFkbWluQHBrYm1zYWxhZml5YWguY29tIiwiZnVsbF9uYW1lIjoiQWRtaW4iLCJyb2xlIjoiQWRtaW5pc3RyYXRvciIsImV4cCI6MTc4ODQ5OTMyMX0.UVAO59cUy2hihEORFNCwwxG_E33YxQ5im8glJNBa68Q';

  // 1. Generate token for a user
  console.log('1. Generating QR token...');
  const genRes = await post(
    '/api/v1/auth/qr-tokens/generate',
    {
      user_id: '01a016a8-e386-7cd3-9950-89950a8bf095', // AMIN LISANA (Guru)
      token_type: 'BADGE',
      label: 'Test Badge Guru',
    },
    { Authorization: `Bearer ${adminToken}` }
  );

  console.log('Generate Status:', genRes.status);
  console.log('Generate Response:', genRes.body);

  const rawToken = genRes.body?.data?.raw_token;
  if (!rawToken) {
    console.error('No raw_token returned!');
    return;
  }

  // 2. Test qr-login with rawToken
  console.log('\n2. Testing POST /api/v1/auth/qr-login with raw_token:', rawToken);
  const loginRes = await post('/api/v1/auth/qr-login', { token: rawToken });
  console.log('Login Status:', loginRes.status);
  console.log('Login Response:', JSON.stringify(loginRes.body, null, 2));
}

testQrLogin();
