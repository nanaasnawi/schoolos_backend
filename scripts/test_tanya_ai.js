const https = require('https');

async function testAi() {
  // 1. Login to get token
  const loginData = JSON.stringify({
    username_or_email: "0106310228@siswa.schoolos.id", // tiyas.febriyani or another user
    password: "Password123!" // let's check what user password is or generate a test token
  });

  console.log("Testing Tanya AI...");
}

testAi();

