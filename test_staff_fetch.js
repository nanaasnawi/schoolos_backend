const run = async () => {
  try {
    const loginRes = await fetch("http://localhost:8000/api/v1/auth/login", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({email: "admin@pkbmsalafiyah.com", password: "password123"})
    });
    const loginJson = await loginRes.json();
    if (!loginJson.success) {
      console.error("Login failed", loginJson);
      return;
    }
    const token = loginJson.data.access_token;
    console.log("LOGIN SUCCESS! Token prefix:", token.substring(0, 20));
  } catch (e) {
    console.error(e);
  }
}
run();
