const run = async () => {
  const res = await fetch("http://localhost:8000/api/v1/auth/login", {
    method: "POST",
    headers: {"Content-Type": "application/json"},
    body: JSON.stringify({email: "admin@pkbmsalafiyah.com", password: "secretpassword"})
  });
  const json = await res.json();
  if (json.success) {
    const token = json.data.token;
    const staffRes = await fetch("http://localhost:8000/api/v1/staff?page_size=100", {
      headers: { "Authorization": "Bearer " + token }
    });
    console.log(JSON.stringify(await staffRes.json(), null, 2));
  } else {
    console.log(json);
  }
};
run();
