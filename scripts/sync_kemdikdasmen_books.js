#!/usr/bin/env node
/**
 * ==============================================================================
 * School OS — SIBI Kemendikdasmen Kurikulum Merdeka Book Scraper & Database Syncer
 * ==============================================================================
 * 
 * Mengambil katalog buku teks resmi Kurikulum Merdeka langsung dari API resmi SIBI
 * (Pusat Perbukuan, Kemendikdasmen) dan menyimpannya ke database School OS (`library_books`).
 * 
 * Penggunaan:
 *   node backend/scripts/sync_kemdikdasmen_books.js [OPTIONS]
 * 
 * Opsi:
 *   --db             Langsung sinkronisasi ke database PostgreSQL (Default jika ada DATABASE_URL)
 *   --dry-run        Hanya tes fetch dan tampilkan ringkasan data tanpa insert ke DB
 *   --export-sql     Hasilkan file SQL seed (backend/migrations/seed_sibi_books.sql)
 *   --export-json    Simpan katalog lengkap dalam format JSON (sibi_books_catalog.json)
 *   --level=JENJANG  Filter jenjang: SMA, SMP, SD, SMK (contoh: --level=SMA)
 *   --class=KELAS    Filter kelas: 10,11,12 (contoh: --class=10,11,12)
 *   --type=TIPE      Filter tipe buku: 'siswa' (Buku Siswa saja), 'guru' (Buku Guru saja)
 *   --limit=N        Batasi jumlah buku (default: semua ~662 buku)
 *   --download-dir=P Unduh file fisik PDF ke folder lokal (contoh: --download-dir=./pdf_books)
 * ==============================================================================
 */

const fs = require('fs');
const path = require('path');
const { Client } = require('pg');
const { pipeline } = require('stream/promises');

const SIBI_API_URL = 'https://api.buku.kemendikdasmen.go.id/api/catalogue/getPenggerakTextBooks?limit=800&offset=0';
const CDN_TARGET_DOMAIN = 'static-sc.cloudapp.web.id';
const DEFAULT_DB_URL = process.env.DATABASE_URL || 'postgres://school_admin:secretpassword@localhost:5433/school_os';

// Parsing argumen CLI
const args = process.argv.slice(2);
const isDryRun = args.includes('--dry-run');
const exportSql = args.includes('--export-sql');
const exportJson = args.includes('--export-json');
const levelArg = (args.find(a => a.startsWith('--level=')) || '').split('=')[1]?.toUpperCase();
const classArg = (args.find(a => a.startsWith('--class=')) || '').split('=')[1];
const typeArg = (args.find(a => a.startsWith('--type=')) || '').split('=')[1]?.toLowerCase();
const limitArg = parseInt((args.find(a => a.startsWith('--limit=')) || '').split('=')[1] || '0', 10);
const downloadDirArg = (args.find(a => a.startsWith('--download-dir=')) || '').split('=')[1];

function sanitizeUrl(url) {
  if (!url) return null;
  // Ganti domain lama yang mati (static.buku.kemdikbud.go.id) ke CDN aktif (static-sc.cloudapp.web.id)
  return url
    .replace('static.buku.kemdikbud.go.id', CDN_TARGET_DOMAIN)
    .replace('static.sc.cloudapp.web.id', CDN_TARGET_DOMAIN);
}

function normalizeSubjectPattern(subjectSlug, title) {
  const text = `${subjectSlug || ''} ${title || ''}`.toLowerCase();
  
  if (text.includes('matematika')) return '%Matematika%';
  if (text.includes('bahasa indonesia') || text.includes('indonesia')) return '%Bahasa Indonesia%';
  if (text.includes('bahasa inggris') || text.includes('inggris')) return '%Bahasa Inggris%';
  if (text.includes('fisika')) return '%Fisika%';
  if (text.includes('biologi')) return '%Biologi%';
  if (text.includes('kimia')) return '%Kimia%';
  if (text.includes('informatika') || text.includes('komputer') || text.includes('koding')) return '%Komputer%';
  if (text.includes('pancasila') || text.includes('ppkn')) return '%Pancasila%';
  if (text.includes('sejarah')) return '%Sejarah%';
  if (text.includes('geografi')) return '%Geografi%';
  if (text.includes('ekonomi')) return '%Ekonomi%';
  if (text.includes('sosiologi')) return '%Sosiologi%';
  if (text.includes('antropologi')) return '%Antropologi%';
  if (text.includes('pjok') || text.includes('jasmani')) return '%Jasmani%';
  if (text.includes('agama islam') || text.includes('pendidikan agama islam')) return '%Agama Islam%';
  if (text.includes('seni musik') || text.includes('seni rupa') || text.includes('seni tari') || text.includes('seni teater') || text.includes('seni budaya')) return '%Seni Budaya%';
  if (text.includes('ipas') || text.includes('alam dan sosial')) return '%IPAS%';
  if (text.includes('ipa')) return '%IPA%';
  if (text.includes('ips')) return '%IPS%';
  return null;
}

async function fetchSibiCatalog() {
  console.log(`\n📡 Mengambil katalog Kurikulum Merdeka dari SIBI Kemendikdasmen...`);
  console.log(`   URL: ${SIBI_API_URL}`);
  
  const response = await fetch(SIBI_API_URL, {
    headers: {
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) SchoolOS-Sync/1.0',
      'Accept': 'application/json, text/plain, */*',
      'Referer': 'https://buku.kemendikdasmen.go.id/'
    }
  });

  if (!response.ok) {
    throw new Error(`Gagal fetch API SIBI: HTTP ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  const rawBooks = data.results || [];
  console.log(`✅ Berhasil mengambil ${rawBooks.length} buku dari SIBI.\n`);
  return rawBooks;
}

function filterBooks(rawBooks) {
  return rawBooks.filter(book => {
    // Validasi URL unduhan PDF
    if (!book.attachment) return false;

    // Filter Jenjang
    if (levelArg) {
      const bookLevel = (book.level || '').toUpperCase();
      if (!bookLevel.includes(levelArg)) return false;
    }

    // Filter Kelas
    if (classArg) {
      const targetClasses = classArg.split(',').map(s => s.trim());
      const bookClass = String(book.class || '').trim();
      if (!targetClasses.includes(bookClass)) return false;
    }

    // Filter Tipe: Siswa / Guru
    if (typeArg === 'siswa') {
      const isGuide = (book.title || '').toLowerCase().includes('panduan guru') || 
                      (book.title || '').toLowerCase().includes('buku guru') ||
                      (book.attachment || '').includes('_BG_');
      if (isGuide) return false;
    } else if (typeArg === 'guru') {
      const isGuide = (book.title || '').toLowerCase().includes('panduan guru') || 
                      (book.title || '').toLowerCase().includes('buku guru') ||
                      (book.attachment || '').includes('_BG_');
      if (!isGuide) return false;
    }

    return true;
  });
}

async function syncToDatabase(books) {
  console.log(`🔌 Menghubungkan ke database PostgreSQL...`);
  const client = new Client({ connectionString: DEFAULT_DB_URL });
  await client.connect();
  console.log(`✅ Terhubung ke database: ${DEFAULT_DB_URL.replace(/:[^:@]+@/, ':***@')}`);

  try {
    // Pastikan tabel library_books sudah ada
    await client.query(`
      CREATE TABLE IF NOT EXISTS library_books (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
        title VARCHAR(255) NOT NULL,
        author VARCHAR(255),
        publisher VARCHAR(255),
        subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
        grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
        total_pages INTEGER NOT NULL DEFAULT 100,
        cover_url TEXT,
        file_url TEXT,
        created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
      );
      CREATE INDEX IF NOT EXISTS idx_library_books_tenant ON library_books(tenant_id);
      CREATE INDEX IF NOT EXISTS idx_library_books_subject ON library_books(subject_id);
    `);

    // Ambil daftar tenant
    const tenantsRes = await client.query('SELECT id, name FROM tenants');
    const tenants = tenantsRes.rows;
    console.log(`🏢 Ditemukan ${tenants.length} tenant sekolah.`);

    let totalInserted = 0;
    let totalSkipped = 0;

    for (const tenant of tenants) {
      console.log(`\n📌 Memproses Tenant: ${tenant.name} (${tenant.id})`);
      
      // Ambil data subjects & grade_levels untuk tenant ini
      const subjectsRes = await client.query('SELECT id, name FROM subjects WHERE tenant_id = $1', [tenant.id]);
      const subjects = subjectsRes.rows;

      const gradeLevelsRes = await client.query('SELECT id, level, name FROM grade_levels WHERE tenant_id = $1', [tenant.id]);
      const gradeLevels = gradeLevelsRes.rows;

      let tenantInserted = 0;
      let tenantSkipped = 0;

      for (const book of books) {
        const title = (book.title || 'Tanpa Judul').trim().substring(0, 255);
        const author = (book.writer || 'Pusat Perbukuan Kemendikdasmen').trim().substring(0, 255);
        const publisher = (book.publisher || 'Pusat Perbukuan, Kemendikdasmen').trim().substring(0, 255);
        const fileUrl = sanitizeUrl(book.attachment);
        const coverUrl = sanitizeUrl(book.image);
        const bookClass = parseInt(book.class, 10);

        // Matching Grade Level
        let gradeLevelId = null;
        if (!isNaN(bookClass)) {
          const matchedGl = gradeLevels.find(gl => gl.level === bookClass);
          if (matchedGl) gradeLevelId = matchedGl.id;
        }

        // Matching Subject
        let subjectId = null;
        const pattern = normalizeSubjectPattern(book.subject, book.title);
        if (pattern) {
          const keyword = pattern.replace(/%/g, '').toLowerCase();
          const matchedSubject = subjects.find(s => s.name.toLowerCase().includes(keyword));
          if (matchedSubject) subjectId = matchedSubject.id;
        }

        // Cek apakah buku sudah ada di database untuk tenant ini
        const checkRes = await client.query(
          `SELECT id FROM library_books WHERE tenant_id = $1 AND (file_url = $2 OR title = $3) LIMIT 1`,
          [tenant.id, fileUrl, title]
        );

        if (checkRes.rows.length > 0) {
          tenantSkipped++;
          continue;
        }

        // Insert buku baru
        await client.query(
          `INSERT INTO library_books (
            id, tenant_id, title, author, publisher, subject_id, grade_level_id, total_pages, cover_url, file_url, created_at, updated_at
          ) VALUES (
            gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW()
          )`,
          [
            tenant.id,
            title,
            author,
            publisher,
            subjectId,
            gradeLevelId,
            book.collation && parseInt(book.collation, 10) > 0 ? parseInt(book.collation, 10) : 150,
            coverUrl,
            fileUrl
          ]
        );
        tenantInserted++;
      }

      console.log(`   ✨ Tenant ${tenant.name}: +${tenantInserted} dimasukkan, ${tenantSkipped} sudah ada.`);
      totalInserted += tenantInserted;
      totalSkipped += tenantSkipped;
    }

    console.log(`\n======================================================`);
    console.log(`🎉 SINKRONISASI SELESAI!`);
    console.log(`   Total Buku Berhasil Disimpan : ${totalInserted}`);
    console.log(`   Total Buku Terlewati (Sudah Ada): ${totalSkipped}`);
    console.log(`======================================================\n`);

  } finally {
    await client.end();
  }
}

function generateSqlSeed(books) {
  const outputPath = path.join(__dirname, '..', 'migrations', 'seed_sibi_books.sql');
  console.log(`💾 Membuat file SQL seed: ${outputPath}...`);

  const escapeSql = (str) => {
    if (!str) return "NULL";
    return "'" + str.replace(/'/g, "''").trim() + "'";
  };

  const valuesRows = books.map(book => {
    const title = (book.title || 'Tanpa Judul').substring(0, 255);
    const author = (book.writer || 'Pusat Perbukuan').substring(0, 255);
    const publisher = (book.publisher || 'Pusat Perbukuan Kemendikdasmen').substring(0, 255);
    const fileUrl = sanitizeUrl(book.attachment);
    const coverUrl = sanitizeUrl(book.image);
    const bookClass = isNaN(parseInt(book.class, 10)) ? 'NULL' : parseInt(book.class, 10);
    const pattern = normalizeSubjectPattern(book.subject, book.title) || '%Umum%';

    return `    (
        ${escapeSql(title)},
        ${escapeSql(author)},
        ${escapeSql(publisher)},
        '${pattern}',
        ${bookClass},
        ${book.collation && parseInt(book.collation, 10) > 0 ? parseInt(book.collation, 10) : 150},
        ${escapeSql(coverUrl)},
        ${escapeSql(fileUrl)}
    )`;
  });

  const sqlContent = `-- Auto-generated SIBI Kurikulum Merdeka seed
-- Created: ${new Date().toISOString()}
-- Total Books: ${books.length}

INSERT INTO library_books (id, tenant_id, title, author, publisher, subject_id, grade_level_id, total_pages, cover_url, file_url, created_at, updated_at)
SELECT
    gen_random_uuid(),
    t.id,
    v.title,
    v.author,
    v.publisher,
    (SELECT s.id FROM subjects s WHERE s.tenant_id = t.id AND s.name ILIKE v.subject_pattern LIMIT 1),
    (SELECT gl.id FROM grade_levels gl WHERE gl.tenant_id = t.id AND gl.level = v.book_class LIMIT 1),
    v.total_pages,
    v.cover_url,
    v.file_url,
    NOW(),
    NOW()
FROM tenants t
CROSS JOIN (
    VALUES
${valuesRows.join(',\n')}
) AS v(title, author, publisher, subject_pattern, book_class, total_pages, cover_url, file_url)
WHERE NOT EXISTS (
    SELECT 1 FROM library_books lb
    WHERE lb.tenant_id = t.id AND (lb.file_url = v.file_url OR lb.title = v.title)
);
`;

  fs.writeFileSync(outputPath, sqlContent, 'utf-8');
  console.log(`✅ File SQL berhasil dibuat: ${outputPath} (${(fs.statSync(outputPath).size / 1024).toFixed(1)} KB)`);
}

function generateJsonCatalog(books) {
  const outputPath = path.join(__dirname, '..', 'scratch', 'sibi_books_catalog.json');
  console.log(`💾 Menyimpan katalog JSON: ${outputPath}...`);

  const processed = books.map(b => ({
    id: b.id,
    title: b.title,
    level: b.level,
    class: b.class,
    subject: b.subject,
    writer: b.writer,
    publisher: b.publisher,
    cover_url: sanitizeUrl(b.image),
    file_url: sanitizeUrl(b.attachment),
    curriculum: b.curriculum
  }));

  fs.writeFileSync(outputPath, JSON.stringify(processed, null, 2), 'utf-8');
  console.log(`✅ File JSON berhasil dibuat: ${outputPath} (${processed.length} buku)`);
}

async function downloadPdfs(books, targetDir) {
  const resolvedDir = path.resolve(process.cwd(), targetDir);
  if (!fs.existsSync(resolvedDir)) {
    fs.mkdirSync(resolvedDir, { recursive: true });
  }
  console.log(`\n📥 Memulai pengunduhan ${books.length} file PDF ke: ${resolvedDir}`);

  let downloadedCount = 0;
  let skippedCount = 0;

  for (let i = 0; i < books.length; i++) {
    const book = books[i];
    const fileUrl = sanitizeUrl(book.attachment);
    if (!fileUrl) continue;

    const fileName = path.basename(new URL(fileUrl).pathname);
    const destPath = path.join(resolvedDir, fileName);

    if (fs.existsSync(destPath) && fs.statSync(destPath).size > 0) {
      skippedCount++;
      continue;
    }

    process.stdout.write(`   [${i + 1}/${books.length}] Mengunduh: ${fileName}... `);
    try {
      const res = await fetch(fileUrl);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const fileStream = fs.createWriteStream(destPath);
      const { Readable } = require('stream');
      await pipeline(Readable.fromWeb(res.body), fileStream);
      const sizeMb = (fs.statSync(destPath).size / (1024 * 1024)).toFixed(1);
      console.log(`✅ Selesai (${sizeMb} MB)`);
      downloadedCount++;
    } catch (err) {
      console.log(`❌ Gagal: ${err.message}`);
    }
  }

  console.log(`\n📁 Unduhan Selesai: ${downloadedCount} diunduh, ${skippedCount} sudah ada sebelumnya.`);
}

async function main() {
  try {
    const rawBooks = await fetchSibiCatalog();
    let books = filterBooks(rawBooks);

    if (limitArg > 0) {
      books = books.slice(0, limitArg);
      console.log(`🔢 Dibatasi hingga ${limitArg} buku pertama.`);
    }

    console.log(`📚 Buku siap diproses: ${books.length} buku.`);
    if (books.length === 0) {
      console.log(`⚠️ Tidak ada buku yang cocok dengan filter yang ditentukan.`);
      return;
    }

    // Tampilkan 3 contoh buku yang ditemukan
    console.log(`\n📋 Contoh buku hasil parsing:`);
    books.slice(0, 3).forEach((b, idx) => {
      console.log(`   ${idx + 1}. [Kelas ${b.class} ${b.level}] ${b.title}`);
      console.log(`      PDF  : ${sanitizeUrl(b.attachment)}`);
      console.log(`      Cover: ${sanitizeUrl(b.image)}`);
    });

    if (exportJson) {
      generateJsonCatalog(books);
    }

    if (exportSql) {
      generateSqlSeed(books);
    }

    if (downloadDirArg) {
      await downloadPdfs(books, downloadDirArg);
    }

    if (!isDryRun && !exportSql && !exportJson && !downloadDirArg) {
      await syncToDatabase(books);
    } else if (isDryRun) {
      console.log(`\n💡 Mode DRY-RUN selesai. Tidak ada data yang ditulis ke database.`);
    }

  } catch (error) {
    console.error(`\n❌ Terjadi kesalahan:`, error.message);
    process.exit(1);
  }
}

main();
