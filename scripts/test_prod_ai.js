const https = require('https');

function request(options, body) {
  return new Promise((resolve, reject) => {
    const req = https.request(options, res => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => resolve({ status: res.statusCode, data: JSON.parse(data) }));
    });
    req.on('error', reject);
    if (body) req.write(JSON.stringify(body));
    req.end();
  });
}

async function run() {
  try {
    // 1. Login as teacher or student or principal
    console.log("Logging in to Railway...");
    const loginRes = await request({
      hostname: 'schoolosbackend-production.up.railway.app',
      path: '/api/v1/auth/login',
      method: 'POST',
      headers: { 'Content-Type': 'application/json' }
    }, {
      identifier: 'guru@schoolos.id',
      password: 'Password123!'
    });

    console.log("Login status:", loginRes.status);
    let token = loginRes.data?.data?.access_token || loginRes.data?.access_token;
    if (!token) {
      console.log("Login response:", loginRes.data);
      // Let's try username/password from database
      return;
    }

    console.log("Got token! Calling tanya-ai...");
    const aiRes = await request({
      hostname: 'schoolosbackend-production.up.railway.app',
      path: '/api/v1/ai/tanya-ai',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`
      }
    }, {
      question: "Jelaskan apa itu fotosintesis secara singkat",
      subject_name: "IPA",
      grade_level: "PAKET A 4"
    });

    console.log("AI status:", aiRes.status);
    console.log("AI data:", JSON.stringify(aiRes.data, null, 2));
  } catch (e) {
    console.error("Error:", e);
  }
}

run();

