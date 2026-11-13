use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::controller::calculate_level;
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    infrastructure::nvidia_ai::{call_nvidia_nim, ChatMessage},
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, InfrastructureError};

#[derive(Debug, Deserialize)]
pub struct DailyQuestQuery {
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitDailyQuestPayload {
    pub quest_id: Uuid,
    pub correct_count: i32,
    pub total_count: i32,
    #[serde(default)]
    pub max_combo: i32,
    #[serde(default)]
    pub duration_seconds: i32,
}

#[derive(Debug, Serialize)]
pub struct DailyQuestLeaderboardEntry {
    pub student_name: String,
    pub class_name: String,
    pub score_percent: i32,
    pub duration_seconds: i32,
    pub xp_earned: i32,
    pub is_me: bool,
}

#[derive(Debug, Serialize)]
pub struct DailyQuestAttemptSummary {
    pub score_percent: i32,
    pub correct_count: i32,
    pub total_count: i32,
    pub max_combo: i32,
    pub duration_seconds: i32,
    pub xp_earned: i32,
}

#[derive(Debug, Serialize)]
pub struct DailyQuestModeInfo {
    pub code: String,
    pub label: String,
    pub is_featured_today: bool,
    pub completed_today: bool,
}

#[derive(Debug, Serialize)]
pub struct DailyQuestResponse {
    pub quest_id: Uuid,
    pub quest_date: NaiveDate,
    pub day_label: String,
    pub grade_band: String,
    pub class_name: String,
    pub game_mode: String,
    pub featured_mode_today: String,
    pub is_featured_today: bool,
    pub title: String,
    pub subtitle: String,
    pub subject_name: String,
    pub source_label: String,
    pub xp_reward: i32,
    pub challenges: Value,
    pub already_completed: bool,
    pub my_attempt: Option<DailyQuestAttemptSummary>,
    pub streak_days: i32,
    pub total_xp: i64,
    pub level: i32,
    pub available_modes: Vec<DailyQuestModeInfo>,
    pub leaderboard: Vec<DailyQuestLeaderboardEntry>,
}

#[derive(Debug, Serialize)]
pub struct SubmitDailyQuestResponse {
    pub awarded: bool,
    pub already_completed: bool,
    pub xp_earned: i32,
    pub score_percent: i32,
    pub total_xp: i64,
    pub level: i32,
    pub streak_days: i32,
    pub leveled_up: bool,
    pub is_preview_account: bool,
    pub message: String,
}

fn wib_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn indonesian_day_label(date: NaiveDate) -> String {
    let day = match date.weekday().num_days_from_monday() {
        0 => "Senin",
        1 => "Selasa",
        2 => "Rabu",
        3 => "Kamis",
        4 => "Jumat",
        5 => "Sabtu",
        _ => "Minggu",
    };
    let month = match date.month() {
        1 => "Januari",
        2 => "Februari",
        3 => "Maret",
        4 => "April",
        5 => "Mei",
        6 => "Juni",
        7 => "Juli",
        8 => "Agustus",
        9 => "September",
        10 => "Oktober",
        11 => "November",
        _ => "Desember",
    };
    format!("{}, {} {} {}", day, date.day(), month, date.year())
}

fn featured_mode_for_date(date: NaiveDate) -> &'static str {
    match date.weekday().num_days_from_monday() {
        0 | 3 => "FACT_OR_MYTH",
        1 | 4 => "CASE_DETECTIVE",
        2 | 5 => "SEQUENCE_ORDER",
        _ => "CONCEPT_GUESS",
    }
}

fn normalize_mode(raw: Option<&str>, fallback: &str) -> String {
    match raw.map(|s| s.trim().to_uppercase()).as_deref() {
        Some("FACT_OR_MYTH") => "FACT_OR_MYTH".to_string(),
        Some("CASE_DETECTIVE") => "CASE_DETECTIVE".to_string(),
        Some("SEQUENCE_ORDER") => "SEQUENCE_ORDER".to_string(),
        Some("CONCEPT_GUESS") => "CONCEPT_GUESS".to_string(),
        _ => fallback.to_string(),
    }
}

fn mode_display_label(code: &str) -> &'static str {
    match code {
        "FACT_OR_MYTH" => "Fakta atau Mitos?",
        "CASE_DETECTIVE" => "Detektif Kasus",
        "SEQUENCE_ORDER" => "Rangkai Logika",
        "CONCEPT_GUESS" => "Tebak Istilah",
        _ => "Misi Harian",
    }
}

fn detect_grade_band(class_name: &str) -> String {
    let upper = class_name.to_uppercase();
    if upper.contains("PAKET A")
        || upper.contains("SD")
        || upper.contains("KELAS 1")
        || upper.contains("KELAS 2")
        || upper.contains("KELAS 3")
        || upper.contains("KELAS 4")
        || upper.contains("KELAS 5")
        || upper.contains("KELAS 6")
    {
        "PAKET_A".to_string()
    } else if upper.contains("PAKET B")
        || upper.contains("SMP")
        || upper.contains("KELAS 7")
        || upper.contains("KELAS 8")
        || upper.contains("KELAS 9")
    {
        "PAKET_B".to_string()
    } else {
        "PAKET_C".to_string()
    }
}

async fn ensure_daily_quest_tables(pool: &PgPool) {
    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_quests (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            quest_date DATE NOT NULL,
            grade_band VARCHAR(32) NOT NULL DEFAULT 'PAKET_C',
            game_mode VARCHAR(40) NOT NULL,
            title VARCHAR(255) NOT NULL,
            subtitle VARCHAR(255) NOT NULL DEFAULT '',
            subject_name VARCHAR(120) NOT NULL DEFAULT 'Literasi & Numerasi',
            source_label VARCHAR(50) NOT NULL DEFAULT 'AI_CURRICULUM',
            xp_reward INT NOT NULL DEFAULT 60,
            challenges_json JSONB NOT NULL DEFAULT '[]'::jsonb,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT uq_daily_quests_tenant_date_band_mode UNIQUE (tenant_id, quest_date, grade_band, game_mode)
        )
        "#,
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_quest_attempts (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            quest_id UUID NOT NULL REFERENCES daily_quests(id) ON DELETE CASCADE,
            student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
            correct_count INT NOT NULL DEFAULT 0,
            total_count INT NOT NULL DEFAULT 0,
            score_percent INT NOT NULL DEFAULT 0,
            max_combo INT NOT NULL DEFAULT 0,
            duration_seconds INT NOT NULL DEFAULT 0,
            xp_earned INT NOT NULL DEFAULT 0,
            completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT uq_daily_quest_attempt_student UNIQUE (quest_id, student_id)
        )
        "#,
    )
    .execute(pool)
    .await;
}

fn extract_json_object(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Some(v);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end > start {
        serde_json::from_str::<Value>(&trimmed[start..=end]).ok()
    } else {
        None
    }
}

async fn try_generate_ai_quest(
    grade_band: &str,
    game_mode: &str,
    topic_hint: &str,
    subject_hint: &str,
    quest_date: NaiveDate,
) -> Option<(String, String, String, Value)> {
    let level_desc = match grade_band {
        "PAKET_A" => "Paket A (setara SD Kelas 4-6), bahasa ramah anak dan mudah dipahami",
        "PAKET_B" => "Paket B (setara SMP Kelas 7-9), kontekstual kehidupan remaja/warga belajar",
        _ => "Paket C (setara SMA Kelas 10-12), kritis, aplikatif untuk dunia kerja/UMKM/kehidupan dewasa",
    };

    let schema_instruction = match game_mode {
        "FACT_OR_MYTH" => r#"Format JSON WAJIB:
{
  "title": "judul singkat seru (max 50 karakter)",
  "subtitle": "subjudul 1 kalimat",
  "subject_name": "nama mapel",
  "challenges": [
    {
      "id": "c1",
      "statement": "pernyataan singkat jelas (max 140 karakter)",
      "is_fact": true,
      "explanation": "penjelasan ringkas 1-2 kalimat",
      "topic_tag": "topik singkat"
    }
  ]
}
Buat TEPAT 5 item di array challenges (campuran is_fact true dan false)."#,
        "CASE_DETECTIVE" => r#"Format JSON WAJIB:
{
  "title": "judul kasus seru (max 50 karakter)",
  "subtitle": "subjudul 1 kalimat",
  "subject_name": "nama mapel",
  "challenges": [
    {
      "id": "c1",
      "scenario": "cerita kasus nyata sehari-hari 2 kalimat",
      "question": "pertanyaan keputusan terbaik",
      "options": ["opsi A", "opsi B", "opsi C", "opsi D"],
      "correct_index": 0,
      "explanation": "penjelasan kenapa opsi tersebut paling tepat",
      "topic_tag": "topik singkat"
    }
  ]
}
Buat TEPAT 4 item di array challenges dengan correct_index bervariasi (0-3)."#,
        "SEQUENCE_ORDER" => r#"Format JSON WAJIB:
{
  "title": "judul misi urut logika (max 50 karakter)",
  "subtitle": "subjudul 1 kalimat",
  "subject_name": "nama mapel",
  "challenges": [
    {
      "id": "c1",
      "prompt": "instruksi menyusun urutan dari awal ke akhir",
      "steps": ["langkah 1 (benar)", "langkah 2 (benar)", "langkah 3 (benar)", "langkah 4 (benar)"],
      "explanation": "penjelasan logika urutan yang benar",
      "topic_tag": "topik singkat"
    }
  ]
}
Buat TEPAT 3 item di array challenges, masing-masing memiliki TEPAT 4 string langkah berurutan yang benar."#,
        _ => r#"Format JSON WAJIB:
{
  "title": "judul tebak istilah harian (max 50 karakter)",
  "subtitle": "subjudul 1 kalimat",
  "subject_name": "nama mapel",
  "challenges": [
    {
      "id": "c1",
      "answer": "SATUKATA",
      "clues": [
        "petunjuk 1 (umum)",
        "petunjuk 2 (lebih spesifik)",
        "petunjuk 3 (sangat membantu)"
      ],
      "acceptable_aliases": ["SATUKATA"],
      "explanation": "definisi singkat istilah tersebut",
      "topic_tag": "topik singkat"
    }
  ]
}
Buat TEPAT 4 item di array challenges. Jawaban wajib 1 kata HURUF KAPITAL tanpa spasi (4-13 huruf)."#,
    };

    let sys_prompt = "Kamu adalah perancang micro-learning game edukatif untuk PKBM Indonesia (Kurikulum Merdeka). Jawab HANYA dengan JSON valid tanpa markdown fence ```json.";
    let user_prompt = format!(
        "Buat Misi Harian tanggal {} untuk jenjang {}.\nKonteks materi aktif sekolah: {} ({}).\nMode game: {}.\n{}",
        quest_date, level_desc, topic_hint, subject_hint, game_mode, schema_instruction
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: sys_prompt,
        },
        ChatMessage {
            role: "user",
            content: &user_prompt,
        },
    ];

    let raw = match tokio::time::timeout(
        std::time::Duration::from_secs(12),
        call_nvidia_nim(messages, 0.7, 1600),
    )
    .await
    {
        Ok(Ok(text)) => text,
        _ => return None,
    };

    let parsed = extract_json_object(&raw)?;
    let challenges = parsed.get("challenges")?.as_array()?;
    if challenges.len() < 2 {
        return None;
    }

    let valid_schema = match game_mode {
        "FACT_OR_MYTH" => challenges.iter().all(|c| {
            c.get("statement").and_then(|v| v.as_str()).is_some()
                && c.get("is_fact").and_then(|v| v.as_bool()).is_some()
        }),
        "CASE_DETECTIVE" => challenges.iter().all(|c| {
            c.get("scenario").and_then(|v| v.as_str()).is_some()
                && c.get("question").and_then(|v| v.as_str()).is_some()
                && c.get("options").and_then(|v| v.as_array()).map(|a| a.len() >= 2).unwrap_or(false)
                && c.get("correct_index").and_then(|v| v.as_u64()).is_some()
        }),
        "SEQUENCE_ORDER" => challenges.iter().all(|c| {
            c.get("prompt").and_then(|v| v.as_str()).is_some()
                && c.get("steps").and_then(|v| v.as_array()).map(|a| a.len() >= 3).unwrap_or(false)
        }),
        _ => challenges.iter().all(|c| {
            c.get("answer").and_then(|v| v.as_str()).map(|s| !s.trim().is_empty()).unwrap_or(false)
                && c.get("clues").and_then(|v| v.as_array()).map(|a| a.len() >= 2).unwrap_or(false)
        }),
    };
    if !valid_schema {
        return None;
    }

    let title = parsed
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Misi Harian Dinamis")
        .trim()
        .to_string();
    let subtitle = parsed
        .get("subtitle")
        .and_then(|v| v.as_str())
        .unwrap_or("Tantangan seru 3 menit untuk mengasah pemahaman hari ini")
        .trim()
        .to_string();
    let subj = parsed
        .get("subject_name")
        .and_then(|v| v.as_str())
        .unwrap_or(subject_hint)
        .trim()
        .to_string();

    Some((title, subtitle, subj, Value::Array(challenges.clone())))
}

fn build_curated_quest(
    grade_band: &str,
    game_mode: &str,
    quest_date: NaiveDate,
    subject_hint: &str,
) -> (String, String, String, Value) {
    let seed = (quest_date.ordinal() as usize) + (quest_date.year() as usize);

    match game_mode {
        "FACT_OR_MYTH" => {
            let pool = vec![
                (
                    "Fakta atau Mitos: Ekonomi & Literasi Sehari-hari",
                    "Geser atau ketuk cepat! Uji logika belanjamu dalam 60 detik.",
                    "Ekonomi & Numerasi",
                    vec![
                        json!({"id":"f1","statement":"Inflasi berarti nilai uang semakin menguat sehingga harga barang cenderung turun terus.","is_fact":false,"explanation":"Mitos! Inflasi justru terjadi saat harga barang naik secara umum sehingga daya beli uang menurun.","topic_tag":"Ekonomi"}),
                        json!({"id":"f2","statement":"Diskon 50% lalu diskon tambahan 20% artinya total potongan harga adalah 70%.","is_fact":false,"explanation":"Mitos! Diskon bertingkat dihitung dari harga setelah diskon pertama: 50% + (20% x 50%) = 60%.","topic_tag":"Numerasi"}),
                        json!({"id":"f3","statement":"Kalimat utama dalam paragraf deduktif terletak di bagian awal paragraf.","is_fact":true,"explanation":"Fakta! Paragraf deduktif dimulai dari gagasan utama lalu diikuti kalimat penjelas.","topic_tag":"Bahasa Indonesia"}),
                        json!({"id":"f4","statement":"Energi matahari termasuk sumber daya alam yang dapat diperbarui karena tersedia terus secara alami.","is_fact":true,"explanation":"Fakta! Energi surya, angin, dan air adalah energi terbarukan ramah lingkungan.","topic_tag":"IPA"}),
                        json!({"id":"f5","statement":"Gotong royong dan musyawarah mufakat merupakan cerminan nilai sila ke-4 Pancasila dalam masyarakat.","is_fact":true,"explanation":"Fakta! Musyawarah untuk mencapai mufakat berakar langsung pada sila ke-4 Pancasila.","topic_tag":"PPKn"}),
                        json!({"id":"f6","statement":"Semua bakteri di tubuh manusia selalu berbahaya dan harus dimusnahkan seluruhnya.","is_fact":false,"explanation":"Mitos! Ada bakteri baik di usus yang justru membantu mencerna makanan dan menjaga imunitas.","topic_tag":"Biologi"}),
                        json!({"id":"f7","statement":"Dalam usaha kecil, omzet penjualan sama artinya dengan laba bersih yang boleh langsung dihabiskan.","is_fact":false,"explanation":"Mitos! Omzet adalah total uang masuk kotor; laba bersih didapat setelah dikurangi seluruh modal dan biaya.","topic_tag":"Kewirausahaan"}),
                    ],
                ),
                (
                    "Fakta atau Mitos: Sains, Bahasa & Warga Cerdas",
                    "Jangan sampai terkecoh! Bedakan fakta ilmiah dari anggapan keliru.",
                    "Lintas Mapel",
                    vec![
                        json!({"id":"g1","statement":"Air mendidih di puncak gunung yang sangat tinggi pada suhu lebih rendah dari 100°C.","is_fact":true,"explanation":"Fakta! Tekanan udara yang lebih rendah di dataran tinggi membuat titik didih air menurun.","topic_tag":"Fisika"}),
                        json!({"id":"g2","statement":"Kata 'di' pada 'di rumah' ditulis terpisah karena menunjukkan kata depan tempat.","is_fact":true,"explanation":"Fakta! 'Di' sebagai preposisi penunjuk tempat ditulis terpisah, berbeda dengan imbuhan pasif seperti 'dimakan'.","topic_tag":"Bahasa Indonesia"}),
                        json!({"id":"g3","statement":"Nilai tukar Rupiah melemah selalu menguntungkan warga yang membeli bahan baku impor.","is_fact":false,"explanation":"Mitos! Saat Rupiah melemah, harga barang impor justru menjadi lebih mahal dalam Rupiah.","topic_tag":"Ekonomi"}),
                        json!({"id":"g4","statement":"Fotosintesis pada tumbuhan hijau melepaskan gas oksigen ke udara.","is_fact":true,"explanation":"Fakta! Tumbuhan menyerap CO2 + air dan melepaskan oksigen dengan bantuan cahaya matahari.","topic_tag":"IPA"}),
                        json!({"id":"g5","statement":"Pajak yang dibayar warga negara hanya digunakan untuk gaji pegawai dan tidak pernah untuk jalan atau sekolah.","is_fact":false,"explanation":"Mitos! APBN dari pajak membiayai infrastruktur publik, subsidi pendidikan, kesehatan, dan fasilitas umum.","topic_tag":"IPS"}),
                        json!({"id":"g6","statement":"Password akun belajar sebaiknya sama persis dengan tanggal lahir agar mudah diingat.","is_fact":false,"explanation":"Mitos! Tanggal lahir mudah ditebak; gunakan kombinasi huruf, angka, dan karakter unik.","topic_tag":"Informatika"}),
                    ],
                ),
            ];
            let chosen = &pool[seed % pool.len()];
            let mut items = chosen.3.clone();
            let rotate = seed % items.len();
            items.rotate_left(rotate);
            items.truncate(5);
            let title = if grade_band == "PAKET_A" {
                format!("Fakta atau Mitos Hari Ini ({})", chosen.2)
            } else {
                chosen.0.to_string()
            };
            (title, chosen.1.to_string(), chosen.2.to_string(), Value::Array(items))
        }
        "CASE_DETECTIVE" => {
            let pool = vec![
                json!({
                    "id": "d1",
                    "scenario": "Warung kopi milik Riko menjual 40 gelas per hari seharga Rp8.000/gelas. Biaya bahan per gelas Rp4.500, sedangkan sewa listrik & lapak Rp60.000/hari.",
                    "question": "Berapa keuntungan bersih Riko dalam 1 hari?",
                    "options": ["Rp80.000", "Rp140.000", "Rp320.000", "Rp200.000"],
                    "correct_index": 0,
                    "explanation": "Margin per gelas = Rp8.000 - Rp4.500 = Rp3.500. Total margin 40 gelas = Rp140.000. Dikurangi biaya tetap Rp60.000 = Rp80.000 bersih!",
                    "topic_tag": "Numerasi Usaha"
                }),
                json!({
                    "id": "d2",
                    "scenario": "Kamu menerima pesan di grup keluarga berisi tautan undian berhadiah resmi pemerintah yang meminta foto KTP dan kode OTP SMS dalam 10 menit.",
                    "question": "Tindakan detektif digital yang paling tepat adalah...",
                    "options": [
                        "Mengabaikan tautan, tidak membagikan KTP/OTP, dan mengedukasi grup bahwa itu penipuan (phishing)",
                        "Langsung mengisi foto KTP asal belum mengirim kode OTP",
                        "Meneruskan pesan tersebut ke semua teman agar ikut mencoba keberuntungan",
                        "Mengklik tautan dulu untuk melihat apakah tampilannya meyakinkan"
                    ],
                    "correct_index": 0,
                    "explanation": "Lembaga resmi tidak pernah meminta OTP lewat tautan grup. Jangan pernah klik tautan mencurigakan atau menyebar data pribadi.",
                    "topic_tag": "Literasi Digital"
                }),
                json!({
                    "id": "d3",
                    "scenario": "Dalam rapat warga, terdapat usulan perbaikan saluran air. Separuh warga setuju dikerjakan akhir pekan, separuh lagi ingin menyewa tukang agar cepat selesai.",
                    "question": "Sikap yang paling sesuai dengan prinsip demokrasi Pancasila untuk menyelesaikan perbedaan ini adalah...",
                    "options": [
                        "Memfasilitasi musyawarah untuk mencari titik temu, misalnya warga bekerja bakti dan urunan biaya alat bersama",
                        "Memaksa kelompok minoritas mengikuti keputusan ketua RT tanpa diskusi",
                        "Membatalkan rapat dan menunda proyek selamanya",
                        "Membiarkan kedua kubu bertengkar sampai ada yang mengalah"
                    ],
                    "correct_index": 0,
                    "explanation": "Musyawarah mufakat mengutamakan kepentingan bersama serta mencari solusi kreatif yang bisa diterima seluruh pihak.",
                    "topic_tag": "PPKn & Sosial"
                }),
                json!({
                    "id": "d4",
                    "scenario": "Sebuah kalimat iklan berbunyi: 'Gunakan sabun X, dijamin 100% langsung sembuhkan semua penyakit kulit dalam 1 menit tanpa efek samping!'",
                    "question": "Dilihat dari kaidah teks persuasif & literasi kritis, ciri pernyataan tersebut adalah...",
                    "options": [
                        "Klaim berlebihan (hiperbola) tanpa bukti uji klinis sehingga perlu diwaspadai",
                        "Kalimat fakta ilmiah objektif yang pasti benar",
                        "Teks deskripsi objektif berdasarkan kamus",
                        "Kalimat pasif yang sesuai ejaan baku"
                    ],
                    "correct_index": 0,
                    "explanation": "Pernyataan '100% langsung sembuh dalam 1 menit' tanpa rujukan ilmiah merupakan klaim hiperbolis yang tidak realistis.",
                    "topic_tag": "Bahasa Indonesia"
                }),
                json!({
                    "id": "d5",
                    "scenario": "Saat musim kemarau, sumur di desa mulai menyusut sementara limbah rumah tangga tetap dialirkan ke selokan yang mengarah ke sungai.",
                    "question": "Langkah pelestarian air bersih jangka panjang yang paling efektif di lingkungan rumah adalah...",
                    "options": [
                        "Membuat lubang biopori resapan air hujan dan memisahkan limbah detergen sebelum dibuang",
                        "Menyedot air sungai terus-menerus memakai pompa besar",
                        "Membuang sampah organik ke selokan agar cepat hanyut",
                        "Menguras bak mandi setiap dua jam sekali"
                    ],
                    "correct_index": 0,
                    "explanation": "Biopori membantu mengembalikan air hujan ke tanah dan menjaga cadangan air tanah tetap bersih.",
                    "topic_tag": "IPA & Lingkungan"
                }),
            ];
            let mut items = pool;
            let rotate = seed % items.len();
            items.rotate_left(rotate);
            items.truncate(4);
            (
                "Detektif Kasus Nyata: Pecahkan Tantangan Hari Ini".to_string(),
                "Baca kasus kehidupan sehari-hari dan pilih langkah paling cerdas!".to_string(),
                if subject_hint.is_empty() { "Keterampilan Terapan".to_string() } else { subject_hint.to_string() },
                Value::Array(items),
            )
        }
        "SEQUENCE_ORDER" => {
            let pool = vec![
                json!({
                    "id": "s1",
                    "prompt": "Urutkan langkah mengelola keuangan pribadi/keluarga bulanan yang sehat dari awal hingga akhir:",
                    "steps": [
                        "Mencatat seluruh sumber pemasukan tetap bulan ini",
                        "Menyisihkan dana darurat dan kebutuhan pokok wajib",
                        "Mengalokasikan anggaran untuk kebutuhan pendukung/belajar",
                        "Mengevaluasi catatan pengeluaran di akhir bulan"
                    ],
                    "explanation": "Keuangan sehat dimulai dari mengetahui pemasukan, mengamankan kebutuhan pokok & tabungan, lalu mengevaluasi pengeluaran.",
                    "topic_tag": "Literasi Finansial"
                }),
                json!({
                    "id": "s2",
                    "prompt": "Urutkan tahapan metode ilmiah saat melakukan pengamatan sederhana:",
                    "steps": [
                        "Merumuskan masalah yang ingin diamati",
                        "Menyusun dugaan sementara (hipotesis)",
                        "Melakukan percobaan dan mencatat data pengamatan",
                        "Menarik kesimpulan berdasarkan hasil data"
                    ],
                    "explanation": "Metode ilmiah selalu berawal dari masalah, lalu hipotesis, eksperimen/pengumpulan data, hingga kesimpulan.",
                    "topic_tag": "IPA"
                }),
                json!({
                    "id": "s3",
                    "prompt": "Urutkan struktur argumen AREL dalam menulis pendapat yang meyakinkan:",
                    "steps": [
                        "Assertion: Menyatakan pendapat atau klaim utama secara tegas",
                        "Reasoning: Menjelaskan alasan logis mengapa pendapat itu penting",
                        "Evidence: Menyertakan fakta, data, atau contoh nyata pendukung",
                        "Link back: Menghubungkan kembali ke kesimpulan utama"
                    ],
                    "explanation": "Pola AREL (Assertion → Reasoning → Evidence → Link back) membuat argumen runtut dan kuat secara akademis.",
                    "topic_tag": "Bahasa & Penalaran"
                }),
                json!({
                    "id": "s4",
                    "prompt": "Urutkan proses terjadinya siklus air di bumi secara berurutan:",
                    "steps": [
                        "Evaporasi: Air laut dan sungai menguap karena panas matahari",
                        "Kondensasi: Uap air mendingin di udara membentuk gumpalan awan",
                        "Presipitasi: Awan jenuh menjatuhkan titik-titik air sebagai hujan",
                        "Infiltrasi: Air hujan meresap ke tanah dan mengalir kembali ke sungai"
                    ],
                    "explanation": "Siklus hidrologi berputar dari penguapan (evaporasi), pengembunan (kondensasi), hujan (presipitasi), hingga penyerapan (infiltrasi).",
                    "topic_tag": "IPA & Geografi"
                }),
            ];
            let mut items = pool;
            let rotate = seed % items.len();
            items.rotate_left(rotate);
            items.truncate(3);
            (
                "Rangkai Logika: Susun Urutan yang Tepat".to_string(),
                "Atur setiap blok langkah dari posisi 1 sampai 4 agar alurnya logis!".to_string(),
                "Logika & Literasi".to_string(),
                Value::Array(items),
            )
        }
        _ => {
            let pool = vec![
                json!({
                    "id": "w1",
                    "answer": "INFLASI",
                    "clues": [
                        "Istilah populer dalam ilmu ekonomi makro.",
                        "Ditandai dengan naiknya harga kebutuhan pokok secara umum dan terus-menerus.",
                        "Menyebabkan nilai riil uang menjadi turun jika tidak diimbangi kenaikan pendapatan."
                    ],
                    "acceptable_aliases": ["INFLASI"],
                    "explanation": "Inflasi adalah kenaikan harga barang dan jasa secara umum dan terus-menerus dalam periode tertentu.",
                    "topic_tag": "Ekonomi"
                }),
                json!({
                    "id": "w2",
                    "answer": "FOTOSINTESIS",
                    "clues": [
                        "Proses penting yang menjaga ketersediaan oksigen di bumi.",
                        "Hanya dapat dilakukan oleh organisme berklorofil dengan cahaya matahari.",
                        "Mengubah air dan karbon dioksida menjadi glukosa dan oksigen."
                    ],
                    "acceptable_aliases": ["FOTOSINTESIS"],
                    "explanation": "Fotosintesis adalah proses tumbuhan hijau memasak makanan dengan bantuan sinar matahari.",
                    "topic_tag": "Biologi / IPA"
                }),
                json!({
                    "id": "w3",
                    "answer": "HIPOTESIS",
                    "clues": [
                        "Sering muncul di bab awal penelitian atau percobaan IPA.",
                        "Berisi jawaban atau dugaan sementara sebelum data dikumpulkan.",
                        "Kebenarannya masih harus diuji melalui observasi atau eksperimen."
                    ],
                    "acceptable_aliases": ["HIPOTESIS", "HIPOTESA"],
                    "explanation": "Hipotesis adalah dugaan sementara yang bersifat ilmiah dan perlu diuji kebenarannya.",
                    "topic_tag": "Sains & Riset"
                }),
                json!({
                    "id": "w4",
                    "answer": "MUSYAWARAH",
                    "clues": [
                        "Tradisi luhur bangsa Indonesia saat mengambil keputusan bersama.",
                        "Menjadi inti dari Sila ke-4 Pancasila.",
                        "Tujuannya mencapai kesepakatan atau mufakat tanpa paksaan."
                    ],
                    "acceptable_aliases": ["MUSYAWARAH"],
                    "explanation": "Musyawarah adalah pembahasan bersama untuk mencapai keputusan mufakat demi kepentingan bersama.",
                    "topic_tag": "PPKn"
                }),
                json!({
                    "id": "w5",
                    "answer": "SINONIM",
                    "clues": [
                        "Istilah kebahasaan dalam pelajaran Bahasa Indonesia.",
                        "Berkaitan dengan hubungan makna antar dua kata berbeda.",
                        "Contohnya adalah kata 'pandai' dengan 'cerdas', atau 'hemat' dengan 'irit'."
                    ],
                    "acceptable_aliases": ["SINONIM"],
                    "explanation": "Sinonim adalah dua kata atau lebih yang memiliki bentuk berbeda tetapi maknanya sama atau mirip.",
                    "topic_tag": "Bahasa Indonesia"
                }),
            ];
            let mut items = pool;
            let rotate = seed % items.len();
            items.rotate_left(rotate);
            items.truncate(4);
            (
                "Tebak Istilah Harian: Uji Wawasan".to_string(),
                "Pecahkan kata kunci materi hanya dengan sedikit petunjuk untuk XP maksimal!".to_string(),
                "Wawasan Lintas Mapel".to_string(),
                Value::Array(items),
            )
        }
    }
}

pub async fn get_daily_quest(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<DailyQuestQuery>,
) -> Result<Json<ApiResponse<DailyQuestResponse>>, ApiError> {
    let actor = req_ctx.actor.as_ref().ok_or_else(|| {
        ApiError::new(
            ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidToken,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    ensure_daily_quest_tables(&ctx.pool).await;

    let today = wib_today();
    let featured_mode = featured_mode_for_date(today);
    let requested_mode = normalize_mode(query.mode.as_deref(), featured_mode);

    // 1. Resolve student & active class context
    let student_row = sqlx::query(
        r#"
        SELECT s.id, s.full_name, COALESCE(c.name, 'Umum') AS class_name, c.id AS class_id
        FROM students s
        LEFT JOIN enrollments e ON e.student_id = s.id AND e.tenant_id = s.tenant_id AND UPPER(COALESCE(e.status, 'ACTIVE')) = 'ACTIVE'
        LEFT JOIN classes c ON c.id = e.class_id AND c.tenant_id = s.tenant_id
        WHERE (s.user_id = $1 OR s.id = $1) AND s.tenant_id = $2 AND s.deleted_at IS NULL
        ORDER BY e.enrolled_at DESC NULLS LAST
        LIMIT 1
        "#,
    )
    .bind(actor.id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let (student_id_opt, class_name, class_id_opt) = match student_row {
        Some(r) => (
            Some(r.get::<Uuid, _>("id")),
            r.try_get::<String, _>("class_name").unwrap_or_else(|_| "Umum".to_string()),
            r.try_get::<Option<Uuid>, _>("class_id").ok().flatten(),
        ),
        None => (None, "Pratinjau Kelas".to_string(), None),
    };

    let grade_band = detect_grade_band(&class_name);

    // 2. Look up recent curriculum/material hint for dynamic contextual generation
    let topic_row = sqlx::query(
        r#"
        SELECT m.title, COALESCE(sub.name, 'Umum') AS subject_name
        FROM learning_materials m
        LEFT JOIN subjects sub ON sub.id = m.subject_id AND sub.tenant_id = m.tenant_id
        WHERE m.tenant_id = $1
          AND m.deleted_at IS NULL
          AND ($2::uuid IS NULL OR m.class_id IS NULL OR m.class_id = $2)
        ORDER BY m.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(class_id_opt)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let (topic_hint, subject_hint) = match topic_row {
        Some(r) => (
            r.try_get::<String, _>("title").unwrap_or_else(|_| "Pembelajaran Kontekstual".to_string()),
            r.try_get::<String, _>("subject_name").unwrap_or_else(|_| "Literasi & Numerasi".to_string()),
        ),
        None => ("Pembelajaran Kontekstual PKBM".to_string(), "Literasi & Numerasi".to_string()),
    };

    // 3. Check cached daily quest for (tenant, date, grade_band, mode)
    let existing = sqlx::query(
        r#"
        SELECT id, title, subtitle, subject_name, source_label, xp_reward, challenges_json
        FROM daily_quests
        WHERE tenant_id = $1 AND quest_date = $2 AND grade_band = $3 AND game_mode = $4
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(today)
    .bind(&grade_band)
    .bind(&requested_mode)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let (quest_id, title, subtitle, subject_name, source_label, xp_reward, challenges) = match existing {
        Some(r) => (
            r.get::<Uuid, _>("id"),
            r.get::<String, _>("title"),
            r.get::<String, _>("subtitle"),
            r.get::<String, _>("subject_name"),
            r.get::<String, _>("source_label"),
            r.get::<i32, _>("xp_reward"),
            r.get::<Value, _>("challenges_json"),
        ),
        None => {
            let (gen_title, gen_subtitle, gen_subject, gen_challenges, gen_source) =
                match try_generate_ai_quest(&grade_band, &requested_mode, &topic_hint, &subject_hint, today).await {
                    Some((t, sub, subj, ch)) => (t, sub, subj, ch, "AI_CURRICULUM".to_string()),
                    None => {
                        let (t, sub, subj, ch) = build_curated_quest(&grade_band, &requested_mode, today, &subject_hint);
                        (t, sub, subj, ch, "DAILY_BANK".to_string())
                    }
                };

            let base_xp = if requested_mode == featured_mode { 80 } else { 40 };

            let row = sqlx::query(
                r#"
                INSERT INTO daily_quests (id, tenant_id, quest_date, grade_band, game_mode, title, subtitle, subject_name, source_label, xp_reward, challenges_json)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (tenant_id, quest_date, grade_band, game_mode) DO UPDATE SET
                    title = daily_quests.title
                RETURNING id, title, subtitle, subject_name, source_label, xp_reward, challenges_json
                "#,
            )
            .bind(req_ctx.tenant_id)
            .bind(today)
            .bind(&grade_band)
            .bind(&requested_mode)
            .bind(&gen_title)
            .bind(&gen_subtitle)
            .bind(&gen_subject)
            .bind(&gen_source)
            .bind(base_xp)
            .bind(&gen_challenges)
            .fetch_one(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

            (
                row.get::<Uuid, _>("id"),
                row.get::<String, _>("title"),
                row.get::<String, _>("subtitle"),
                row.get::<String, _>("subject_name"),
                row.get::<String, _>("source_label"),
                row.get::<i32, _>("xp_reward"),
                row.get::<Value, _>("challenges_json"),
            )
        }
    };

    // 4. Student attempt + gamification stats + completed modes today
    let mut my_attempt: Option<DailyQuestAttemptSummary> = None;
    let mut completed_modes_today: Vec<String> = Vec::new();
    let mut total_xp: i64 = 0;
    let mut level: i32 = 1;
    let mut streak_days: i32 = 0;

    if let Some(sid) = student_id_opt {
        if let Ok(Some(att)) = sqlx::query(
            r#"
            SELECT score_percent, correct_count, total_count, max_combo, duration_seconds, xp_earned
            FROM daily_quest_attempts
            WHERE quest_id = $1 AND student_id = $2
            LIMIT 1
            "#,
        )
        .bind(quest_id)
        .bind(sid)
        .fetch_optional(&ctx.pool)
        .await
        {
            my_attempt = Some(DailyQuestAttemptSummary {
                score_percent: att.get("score_percent"),
                correct_count: att.get("correct_count"),
                total_count: att.get("total_count"),
                max_combo: att.get("max_combo"),
                duration_seconds: att.get("duration_seconds"),
                xp_earned: att.get("xp_earned"),
            });
        }

        if let Ok(rows) = sqlx::query(
            r#"
            SELECT dq.game_mode
            FROM daily_quest_attempts a
            JOIN daily_quests dq ON dq.id = a.quest_id
            WHERE a.student_id = $1 AND dq.tenant_id = $2 AND dq.quest_date = $3
            "#,
        )
        .bind(sid)
        .bind(req_ctx.tenant_id)
        .bind(today)
        .fetch_all(&ctx.pool)
        .await
        {
            completed_modes_today = rows.into_iter().map(|r| r.get::<String, _>("game_mode")).collect();
        }

        if let Ok(Some(xp_row)) = sqlx::query(
            "SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2 LIMIT 1"
        )
        .bind(sid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        {
            total_xp = xp_row.get("total_xp");
            level = xp_row.get("level");
            streak_days = xp_row.get("streak_days");
        }
    }

    // 5. Daily Leaderboard for this quest
    let lb_rows = sqlx::query(
        r#"
        SELECT
            a.student_id,
            s.full_name AS student_name,
            COALESCE((
                SELECT c.name FROM enrollments e
                JOIN classes c ON c.id = e.class_id
                WHERE e.student_id = s.id AND e.tenant_id = a.tenant_id AND UPPER(COALESCE(e.status, 'ACTIVE')) = 'ACTIVE'
                ORDER BY e.enrolled_at DESC NULLS LAST
                LIMIT 1
            ), '-') AS class_name,
            a.score_percent,
            a.duration_seconds,
            a.xp_earned
        FROM daily_quest_attempts a
        JOIN students s ON s.id = a.student_id
        WHERE a.quest_id = $1 AND a.tenant_id = $2
        ORDER BY a.score_percent DESC, a.max_combo DESC, a.duration_seconds ASC, a.completed_at ASC
        LIMIT 10
        "#,
    )
    .bind(quest_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let leaderboard: Vec<DailyQuestLeaderboardEntry> = lb_rows
        .into_iter()
        .map(|r| {
            let entry_sid: Uuid = r.get("student_id");
            DailyQuestLeaderboardEntry {
                student_name: r.get("student_name"),
                class_name: r.get("class_name"),
                score_percent: r.get("score_percent"),
                duration_seconds: r.get("duration_seconds"),
                xp_earned: r.get("xp_earned"),
                is_me: student_id_opt == Some(entry_sid),
            }
        })
        .collect();

    let all_modes = ["FACT_OR_MYTH", "CASE_DETECTIVE", "SEQUENCE_ORDER", "CONCEPT_GUESS"];
    let available_modes = all_modes
        .iter()
        .map(|code| DailyQuestModeInfo {
            code: (*code).to_string(),
            label: mode_display_label(code).to_string(),
            is_featured_today: *code == featured_mode,
            completed_today: completed_modes_today.iter().any(|m| m == code),
        })
        .collect();

    Ok(Json(ApiResponse::success(
        DailyQuestResponse {
            quest_id,
            quest_date: today,
            day_label: indonesian_day_label(today),
            grade_band,
            class_name,
            game_mode: requested_mode.clone(),
            featured_mode_today: featured_mode.to_string(),
            is_featured_today: requested_mode == featured_mode,
            title,
            subtitle,
            subject_name,
            source_label,
            xp_reward,
            challenges,
            already_completed: my_attempt.is_some(),
            my_attempt,
            streak_days,
            total_xp,
            level,
            available_modes,
            leaderboard,
        },
        req_ctx.request_id,
    )))
}

pub async fn submit_daily_quest(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SubmitDailyQuestPayload>,
) -> Result<Json<ApiResponse<SubmitDailyQuestResponse>>, ApiError> {
    let actor = req_ctx.actor.as_ref().ok_or_else(|| {
        ApiError::new(
            ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidToken,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    ensure_daily_quest_tables(&ctx.pool).await;

    let quest_row = sqlx::query(
        "SELECT id, title, xp_reward, game_mode FROM daily_quests WHERE id = $1 AND tenant_id = $2 LIMIT 1"
    )
    .bind(payload.quest_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::ResourceNotFound,
                "Misi harian tidak ditemukan.".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let quest_title: String = quest_row.get("title");
    let base_reward: i32 = quest_row.get("xp_reward");

    let safe_total = payload.total_count.max(1);
    let safe_correct = payload.correct_count.clamp(0, safe_total);
    let score_percent = ((safe_correct as f64 / safe_total as f64) * 100.0).round() as i32;
    let combo_bonus = (payload.max_combo.max(0) * 5).min(25);
    let completion_bonus = (base_reward / 3).max(15);
    let accuracy_xp = ((base_reward as f64 * (safe_correct as f64 / safe_total as f64)).round() as i32).max(5);
    let computed_xp = (completion_bonus + accuracy_xp + combo_bonus).clamp(20, 140);

    // Resolve actual student row (FK safe)
    let student_opt = sqlx::query(
        "SELECT id FROM students WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1"
    )
    .bind(actor.id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let Some(st_row) = student_opt else {
        return Ok(Json(ApiResponse::success(
            SubmitDailyQuestResponse {
                awarded: false,
                already_completed: false,
                xp_earned: computed_xp,
                score_percent,
                total_xp: 0,
                level: 1,
                streak_days: 1,
                leveled_up: false,
                is_preview_account: true,
                message: format!(
                    "Mode Pratinjau selesai dengan akurasi {}% (simulasi +{} XP).",
                    score_percent, computed_xp
                ),
            },
            req_ctx.request_id,
        )));
    };

    let student_id: Uuid = st_row.get("id");

    // Check if student already completed this quest today
    let existing_attempt = sqlx::query(
        "SELECT xp_earned, score_percent FROM daily_quest_attempts WHERE quest_id = $1 AND student_id = $2 LIMIT 1"
    )
    .bind(payload.quest_id)
    .bind(student_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    if existing_attempt.is_some() {
        // Update best score if improved, no duplicate XP farming
        let _ = sqlx::query(
            r#"
            UPDATE daily_quest_attempts
            SET
                correct_count = GREATEST(correct_count, $3),
                score_percent = GREATEST(score_percent, $4),
                max_combo = GREATEST(max_combo, $5),
                duration_seconds = CASE WHEN $4 > score_percent THEN $6 ELSE LEAST(duration_seconds, $6) END
            WHERE quest_id = $1 AND student_id = $2
            "#,
        )
        .bind(payload.quest_id)
        .bind(student_id)
        .bind(safe_correct)
        .bind(score_percent)
        .bind(payload.max_combo.max(0))
        .bind(payload.duration_seconds.max(1))
        .execute(&ctx.pool)
        .await;

        let cur = sqlx::query("SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2")
            .bind(student_id)
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

        let (total_xp, level, streak_days) = match cur {
            Some(r) => (r.get("total_xp"), r.get("level"), r.get("streak_days")),
            None => (0, 1, 1),
        };

        return Ok(Json(ApiResponse::success(
            SubmitDailyQuestResponse {
                awarded: false,
                already_completed: true,
                xp_earned: 0,
                score_percent,
                total_xp,
                level,
                streak_days,
                leveled_up: false,
                is_preview_account: false,
                message: format!(
                    "Mode Latihan Ulang selesai! Skor {}% (XP hari ini sudah diklaim sebelumnya).",
                    score_percent
                ),
            },
            req_ctx.request_id,
        )));
    }

    // Record first completion & award XP atomically
    let _ = sqlx::query(
        r#"
        INSERT INTO daily_quest_attempts (id, tenant_id, quest_id, student_id, correct_count, total_count, score_percent, max_combo, duration_seconds, xp_earned, completed_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
        ON CONFLICT (quest_id, student_id) DO NOTHING
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(payload.quest_id)
    .bind(student_id)
    .bind(safe_correct)
    .bind(safe_total)
    .bind(score_percent)
    .bind(payload.max_combo.max(0))
    .bind(payload.duration_seconds.max(1))
    .bind(computed_xp)
    .execute(&ctx.pool)
    .await;

    let tx_desc = format!("Misi Harian: {} ({}%)", quest_title, score_percent);
    let _ = sqlx::query(
        r#"
        INSERT INTO student_xp_transactions (id, tenant_id, student_id, action_type, reference_id, xp_amount, description, created_at)
        VALUES (gen_random_uuid(), $1, $2, 'DAILY_QUEST', $3, $4, $5, NOW())
        ON CONFLICT (student_id, action_type, reference_id) DO NOTHING
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(student_id)
    .bind(payload.quest_id)
    .bind(computed_xp)
    .bind(&tx_desc)
    .execute(&ctx.pool)
    .await;

    let current_xp_row = sqlx::query(
        "SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1"
    )
    .bind(student_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let (old_total, old_level, old_streak, last_date) = match current_xp_row {
        Some(r) => (
            r.get::<i64, _>("total_xp"),
            r.get::<i32, _>("level"),
            r.get::<i32, _>("streak_days"),
            r.try_get::<Option<NaiveDate>, _>("last_activity_date").ok().flatten(),
        ),
        None => (0, 1, 0, None),
    };

    let new_total = old_total + (computed_xp as i64);
    let new_level = calculate_level(new_total);
    let today = wib_today();
    let new_streak = match last_date {
        Some(d) if d == today => old_streak.max(1),
        Some(d) if d == today.pred_opt().unwrap_or(today) => old_streak + 1,
        _ => 1,
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO student_xp (student_id, tenant_id, total_xp, level, streak_days, last_activity_date, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (student_id) DO UPDATE SET
            total_xp = EXCLUDED.total_xp,
            level = EXCLUDED.level,
            streak_days = EXCLUDED.streak_days,
            last_activity_date = EXCLUDED.last_activity_date,
            updated_at = NOW()
        "#,
    )
    .bind(student_id)
    .bind(req_ctx.tenant_id)
    .bind(new_total)
    .bind(new_level)
    .bind(new_streak)
    .bind(today)
    .execute(&ctx.pool)
    .await;

    let leveled_up = new_level > old_level;
    let msg = if leveled_up {
        format!("Luar biasa! Kamu naik ke Level {} dan meraih +{} XP!", new_level, computed_xp)
    } else {
        format!("Misi Harian tuntas ({}%)! +{} XP ditambahkan.", score_percent, computed_xp)
    };

    Ok(Json(ApiResponse::success(
        SubmitDailyQuestResponse {
            awarded: true,
            already_completed: false,
            xp_earned: computed_xp,
            score_percent,
            total_xp: new_total,
            level: new_level,
            streak_days: new_streak,
            leveled_up,
            is_preview_account: false,
            message: msg,
        },
        req_ctx.request_id,
    )))
}
