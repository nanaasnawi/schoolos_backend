-- Reset passwords untuk semua admin users ke 'admin123'
-- Hash ini dibuat dengan argon2id, m=19456, t=2, p=1
UPDATE users SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$cWWLRnm9NTpmXtGbRlGUVA$7KjpCqNtmNiOnMXTzZMbmJ4S4oqyi9oMLYrPQ1xbkJ0' WHERE email = 'admin@schoolos.id';
UPDATE users SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$cWWLRnm9NTpmXtGbRlGUVA$7KjpCqNtmNiOnMXTzZMbmJ4S4oqyi9oMLYrPQ1xbkJ0' WHERE email = 'admin@nusantarajaya.id';
UPDATE users SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$cWWLRnm9NTpmXtGbRlGUVA$7KjpCqNtmNiOnMXTzZMbmJ4S4oqyi9oMLYrPQ1xbkJ0' WHERE email = 'admin@harapanbangsa.id';

-- Verifikasi
SELECT email, LEFT(password_hash, 50) as hash_preview FROM users;
