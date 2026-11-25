const https = require('https');

const apiKey = process.env.NVIDIA_API_KEY;
if (!apiKey) {
  throw new Error('NVIDIA_API_KEY environment variable is required');
}

const postData = JSON.stringify({
  model: process.env.NVIDIA_MODEL || 'nvidia/nemotron-3.5-lightning-30b-a3b',
  messages: [{ role: 'user', content: 'Halo, apa kabar?' }],
  max_tokens: 50,
  temperature: 0.7,
});

const options = {
  hostname: 'integrate.api.nvidia.com',
  port: 443,
  path: '/v1/chat/completions',
  method: 'POST',
  headers: {
    Authorization: `Bearer ${apiKey}`,
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(postData),
  },
};

const req = https.request(options, (res) => {
  console.log('Status:', res.statusCode);
  let data = '';
  res.on('data', (chunk) => (data += chunk));
  res.on('end', () => console.log('Body:', data));
});
req.on('error', console.error);
req.write(postData);
req.end();
