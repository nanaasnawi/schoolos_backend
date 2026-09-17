const fs = require('fs');
const path = require('path');

const SIBI_API_URL = 'https://api.buku.kemendikdasmen.go.id/api/catalogue/getPenggerakTextBooks?limit=800&offset=0';
const CDN_TARGET_DOMAIN = 'static-sc.cloudapp.web.id';
const OUTPUT_FILE = path.join(__dirname, '..', 'migrations', '20260917071000_seed_national_curriculum_books.sql');

function sanitizeUrl(url) {
  if (!url) return null;
  return url
    .replace('static.buku.kemdikbud.go.id', CDN_TARGET_DOMAIN)
    .replace('static.sc.cloudapp.web.id', CDN_TARGET_DOMAIN);
}

function normalizeSubject(subjectSlug, title) {
  const text = `${subjectSlug || ''} ${title || ''}`.toLowerCase();
  
  if (text.includes('matematika')) return 'Matematika';
  if (text.includes('bahasa indonesia')) return 'Bahasa Indonesia';
  if (text.includes('bahasa inggris')) return 'Bahasa Inggris';
  if (text.includes('bahasa jepang')) return 'Bahasa Jepang';
  if (text.includes('bahasa jerman')) return 'Bahasa Jerman';
  if (text.includes('bahasa prancis')) return 'Bahasa Prancis';
  if (text.includes('bahasa korea')) return 'Bahasa Korea';
  if (text.includes('bahasa mandarin')) return 'Bahasa Mandarin';
  if (text.includes('bahasa arab')) return 'Bahasa Arab';
  if (text.includes('fisika')) return 'Fisika';
  if (text.includes('biologi')) return 'Biologi';
  if (text.includes('kimia')) return 'Kimia';
  if (text.includes('informatika') || text.includes('koding') || text.includes('kecerdasan artifisial')) return 'Informatika';
  if (text.includes('pancasila') || text.includes('ppkn')) return 'Pendidikan Pancasila';
  if (text.includes('sejarah')) return 'Sejarah';
  if (text.includes('geografi')) return 'Geografi';
  if (text.includes('ekonomi')) return 'Ekonomi';
  if (text.includes('sosiologi')) return 'Sosiologi';
  if (text.includes('antropologi')) return 'Antropologi';
  if (text.includes('pjok') || text.includes('jasmani')) return 'Pendidikan Jasmani';
  if (text.includes('agama islam')) return 'Pendidikan Agama Islam';
  if (text.includes('agama kristen')) return 'Pendidikan Agama Kristen';
  if (text.includes('agama katolik')) return 'Pendidikan Agama Katolik';
  if (text.includes('agama hindu')) return 'Pendidikan Agama Hindu';
  if (text.includes('agama buddha')) return 'Pendidikan Agama Buddha';
  if (text.includes('agama khonghucu')) return 'Pendidikan Agama Khonghucu';
  if (text.includes('kepercayaan')) return 'Pendidikan Kepercayaan';
  if (text.includes('seni musik')) return 'Seni Musik';
  if (text.includes('seni rupa')) return 'Seni Rupa';
  if (text.includes('seni tari')) return 'Seni Tari';
  if (text.includes('seni teater')) return 'Seni Teater';
  if (text.includes('seni budaya')) return 'Seni Budaya';
  if (text.includes('prakarya')) return 'Prakarya dan Kewirausahaan';
  if (text.includes('ipas')) return 'IPAS';
  if (text.includes('ipa')) return 'IPA';
  if (text.includes('ips')) return 'IPS';
  return 'Umum';
}

function getCanonicalSubject(subjectSlug, title) {
  const text = `${subjectSlug || ''} ${title || ''}`.toLowerCase();
  
  if (text.includes('matematika')) return 'MATEMATIKA';
  if (text.includes('bahasa indonesia')) return 'BAHASA_INDONESIA';
  if (text.includes('bahasa inggris')) return 'BAHASA_INGGRIS';
  if (text.includes('bahasa jepang')) return 'BAHASA_JEPANG';
  if (text.includes('bahasa jerman')) return 'BAHASA_JERMAN';
  if (text.includes('bahasa prancis')) return 'BAHASA_PRANCIS';
  if (text.includes('bahasa korea')) return 'BAHASA_KOREA';
  if (text.includes('bahasa mandarin')) return 'BAHASA_MANDARIN';
  if (text.includes('bahasa arab')) return 'BAHASA_ARAB';
  if (text.includes('fisika')) return 'FISIKA';
  if (text.includes('biologi')) return 'BIOLOGI';
  if (text.includes('kimia')) return 'KIMIA';
  if (text.includes('informatika') || text.includes('koding') || text.includes('kecerdasan artifisial')) return 'INFORMATIKA';
  if (text.includes('pancasila') || text.includes('ppkn')) return 'PENDIDIKAN_PANCASILA';
  if (text.includes('sejarah')) return 'SEJARAH';
  if (text.includes('geografi')) return 'GEOGRAFI';
  if (text.includes('ekonomi')) return 'EKONOMI';
  if (text.includes('sosiologi')) return 'SOSIOLOGI';
  if (text.includes('antropologi')) return 'ANTROPOLOGI';
  if (text.includes('pjok') || text.includes('jasmani')) return 'PJOK';
  if (text.includes('agama islam')) return 'PENDIDIKAN_AGAMA_ISLAM';
  if (text.includes('agama kristen')) return 'PENDIDIKAN_AGAMA_KRISTEN';
  if (text.includes('agama katolik')) return 'PENDIDIKAN_AGAMA_KATOLIK';
  if (text.includes('agama hindu')) return 'PENDIDIKAN_AGAMA_HINDU';
  if (text.includes('agama buddha')) return 'PENDIDIKAN_AGAMA_BUDDHA';
  if (text.includes('agama khonghucu')) return 'PENDIDIKAN_AGAMA_KHONGHUCU';
  if (text.includes('seni musik')) return 'SENI_MUSIK';
  if (text.includes('seni rupa')) return 'SENI_RUPA';
  if (text.includes('seni tari')) return 'SENI_TARI';
  if (text.includes('seni teater')) return 'SENI_TEATER';
  if (text.includes('seni budaya') || text.includes('seni')) return 'SENI_BUDAYA';
  if (text.includes('prakarya')) return 'PRAKARYA';
  if (text.includes('ipas')) return 'IPAS';
  if (text.includes('ipa')) return 'IPA';
  if (text.includes('ips')) return 'IPS';
  return 'UMUM';
}

function escapeSql(str) {
  if (!str) return "NULL";
  return "'" + str.replace(/'/g, "''").trim() + "'";
}

async function run() {
  console.log('Fetching SIBI catalog...');
  const res = await fetch(SIBI_API_URL, {
    headers: { 'User-Agent': 'Mozilla/5.0' }
  });
  const data = await res.json();
  const books = data.results || [];
  console.log(`Fetched ${books.length} books.`);

  const validBooks = books.filter(b => b.attachment && b.title);
  console.log(`Valid books with attachments: ${validBooks.length}`);

  const seen = new Set();
  const rows = [];
  for (const b of validBooks) {
    const title = b.title.trim().substring(0, 255);
    const author = (b.writer || 'Pusat Perbukuan, Kemendikdasmen').trim().substring(0, 255);
    const publisher = (b.publisher || 'Pusat Perbukuan Kemendikdasmen').trim().substring(0, 255);
    const subjectName = normalizeSubject(b.subject, b.title);
    const canonicalSubject = getCanonicalSubject(b.subject, b.title);
    const classLevel = parseInt(b.class, 10);
    const validClass = isNaN(classLevel) ? 'NULL' : classLevel;
    const totalPages = b.collation && parseInt(b.collation, 10) > 0 ? parseInt(b.collation, 10) : 150;
    const coverUrl = sanitizeUrl(b.image);
    const fileUrl = sanitizeUrl(b.attachment);

    const key = `${title}:::${fileUrl}`;
    if (seen.has(key)) continue;
    seen.add(key);

    rows.push(`    (${escapeSql(title)}, ${escapeSql(author)}, ${escapeSql(publisher)}, ${escapeSql(subjectName)}, ${escapeSql(canonicalSubject)}, ${validClass}, ${totalPages}, ${escapeSql(coverUrl)}, ${escapeSql(fileUrl)})`);
  }

  const sql = `-- 20260917071000_seed_national_curriculum_books.sql
-- National Kurikulum Merdeka Library Books Seed (Kemendikdasmen)
-- Total Books: ${rows.length}
-- tenant_id is NULL to make them universally accessible to all schools

INSERT INTO library_books (
    id, tenant_id, title, author, publisher, subject_name, canonical_subject, class_level, total_pages, cover_url, file_url, created_at, updated_at
)
SELECT
    gen_random_uuid(), NULL, v.title, v.author, v.publisher, v.subject_name, v.canonical_subject, v.class_level, v.total_pages, v.cover_url, v.file_url, NOW(), NOW()
FROM (
    VALUES
${rows.join(',\n')}
) AS v(title, author, publisher, subject_name, canonical_subject, class_level, total_pages, cover_url, file_url)
WHERE NOT EXISTS (
    SELECT 1 FROM library_books lb WHERE lb.tenant_id IS NULL AND lb.title = v.title AND lb.file_url = v.file_url
);
`;

  fs.writeFileSync(OUTPUT_FILE, sql, 'utf-8');
  console.log(`Successfully generated ${OUTPUT_FILE} (${(fs.statSync(OUTPUT_FILE).size / 1024).toFixed(1)} KB)`);
}

run();
