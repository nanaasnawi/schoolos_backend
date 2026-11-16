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
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, InfrastructureError};

#[derive(Debug, Deserialize)]
pub struct DailyQuestQuery {
    pub mode: Option<String>,
    #[serde(default)]
    pub round: i32,
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
    #[serde(default)]
    pub round_number: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShibaPetStatus {
    pub pet_name: String,
    pub pet_stage: i32,
    pub stage_title: String,
    pub food_points: i32,
    pub points_to_next_stage: i32,
    pub total_fed: i32,
    pub biscuits: i32,
    pub fifty_fifty_tokens: i32,
}

#[derive(Debug, Serialize)]
pub struct FeedPetResponse {
    pub fed: bool,
    pub evolved: bool,
    pub bonus_xp_earned: i32,
    pub total_xp: i64,
    pub level: i32,
    pub pet_status: ShibaPetStatus,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DailyChestResponse {
    pub opened: bool,
    pub already_claimed: bool,
    pub reward_type: String,
    pub reward_amount: i32,
    pub reward_label: String,
    pub total_xp: i64,
    pub level: i32,
    pub pet_status: ShibaPetStatus,
    pub message: String,
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
    pub round_number: i32,
    pub challenges: Value,
    pub already_completed: bool,
    pub my_attempt: Option<DailyQuestAttemptSummary>,
    pub streak_days: i32,
    pub total_xp: i64,
    pub level: i32,
    pub available_modes: Vec<DailyQuestModeInfo>,
    pub leaderboard: Vec<DailyQuestLeaderboardEntry>,
    pub pet_status: Option<ShibaPetStatus>,
    pub chest_claimed_today: bool,
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
    pub biscuits_earned: i32,
    pub pet_status: Option<ShibaPetStatus>,
    pub message: String,
}

const ALL_MODES: [&str; 4] = [
    "CROSSWORD",
    "SLIDING_PUZZLE",
    "BLOCK_PUZZLE",
    "NUMBER_MERGE",
];

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
    match date.weekday().num_days_from_monday() % 4 {
        0 => "CROSSWORD",
        1 => "SLIDING_PUZZLE",
        2 => "BLOCK_PUZZLE",
        _ => "NUMBER_MERGE",
    }
}

fn normalize_mode(raw: Option<&str>, fallback: &str) -> String {
    match raw.map(|s| s.trim().to_uppercase()).as_deref() {
        Some("CROSSWORD") => "CROSSWORD".to_string(),
        Some("SLIDING_PUZZLE") => "SLIDING_PUZZLE".to_string(),
        Some("BLOCK_PUZZLE") => "BLOCK_PUZZLE".to_string(),
        Some("NUMBER_MERGE") => "NUMBER_MERGE".to_string(),
        _ => fallback.to_string(),
    }
}

fn mode_display_label(code: &str) -> &'static str {
    match code {
        "CROSSWORD" => "Teka-Teki Silang",
        "SLIDING_PUZZLE" => "Puzzle Geser",
        "BLOCK_PUZZLE" => "Block Blast",
        "NUMBER_MERGE" => "2048 Angka",
        _ => "Zona Game",
    }
}

fn shiba_stage_title(stage: i32) -> &'static str {
    match stage {
        1 => "Anak Kucing Penasaran",
        2 => "Kucing Pelajar Aktif",
        3 => "Detektif Kucing Pintar",
        _ => "Profesor Kucing Emas",
    }
}

fn build_pet_status(
    pet_stage: i32,
    food_points: i32,
    total_fed: i32,
    biscuits: i32,
    tokens: i32,
) -> ShibaPetStatus {
    let clamped_stage = pet_stage.max(1);
    let points = food_points.clamp(0, 99);
    ShibaPetStatus {
        pet_name: "Ciko Pintar".to_string(),
        pet_stage: clamped_stage,
        stage_title: shiba_stage_title(clamped_stage).to_string(),
        food_points: points,
        points_to_next_stage: (100 - points).max(0),
        total_fed,
        biscuits: biscuits.max(0),
        fifty_fifty_tokens: tokens.max(0),
    }
}

async fn load_or_create_pet(pool: &PgPool, tenant_id: Uuid, student_id: Uuid) -> ShibaPetStatus {
    let _ = sqlx::query(
        r#"
        INSERT INTO student_shiba_pets (student_id, tenant_id, pet_stage, food_points, total_fed, biscuits, fifty_fifty_tokens, updated_at)
        VALUES ($1, $2, 1, 25, 0, 2, 1, NOW())
        ON CONFLICT (student_id) DO NOTHING
        "#,
    )
    .bind(student_id)
    .bind(tenant_id)
    .execute(pool)
    .await;

    let row = sqlx::query(
        "SELECT pet_stage, food_points, total_fed, biscuits, fifty_fifty_tokens FROM student_shiba_pets WHERE student_id = $1 LIMIT 1",
    )
    .bind(student_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some(r) => build_pet_status(
            r.get("pet_stage"),
            r.get("food_points"),
            r.get("total_fed"),
            r.get("biscuits"),
            r.get("fifty_fifty_tokens"),
        ),
        None => build_pet_status(1, 25, 0, 2, 1),
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
            subject_name VARCHAR(120) NOT NULL DEFAULT 'Zona Game Edukasi',
            source_label VARCHAR(50) NOT NULL DEFAULT 'PROCEDURAL_ENGINE',
            xp_reward INT NOT NULL DEFAULT 80,
            challenges_json JSONB NOT NULL DEFAULT '[]'::jsonb,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT uq_daily_quests_tenant_date_band_mode UNIQUE (tenant_id, quest_date, grade_band, game_mode)
        )
        "#,
    )
    .execute(pool)
    .await;

    // Clean up legacy hardcoded trivia modes if still cached in daily_quests
    let _ = sqlx::query(
        "DELETE FROM daily_quests WHERE game_mode NOT IN ('CROSSWORD', 'SLIDING_PUZZLE', 'BLOCK_PUZZLE', 'NUMBER_MERGE')"
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

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS student_shiba_pets (
            student_id UUID PRIMARY KEY REFERENCES students(id) ON DELETE CASCADE,
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            pet_stage INT NOT NULL DEFAULT 1,
            food_points INT NOT NULL DEFAULT 25,
            total_fed INT NOT NULL DEFAULT 0,
            biscuits INT NOT NULL DEFAULT 2,
            fifty_fifty_tokens INT NOT NULL DEFAULT 1,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_quest_chest_claims (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
            claim_date DATE NOT NULL,
            reward_type VARCHAR(40) NOT NULL,
            reward_amount INT NOT NULL DEFAULT 0,
            reward_label TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT uq_daily_quest_chest_claim UNIQUE (student_id, claim_date)
        )
        "#,
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_quest_bonus_logs (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
            bonus_date DATE NOT NULL,
            biscuits_earned INT NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await;
}

fn next_rand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn seed_for_quest(
    date: NaiveDate,
    round: i32,
    actor_id: Uuid,
    mode: &str,
    grade_band: &str,
) -> u64 {
    let mut s = ((date.year() as u64) << 16)
        ^ ((date.ordinal() as u64) << 4)
        ^ ((round.max(0) as u64).wrapping_mul(2654435761));
    if round > 0 {
        s ^= actor_id.as_u128() as u64;
        s ^= (actor_id.as_u128() >> 64) as u64;
    }
    for b in mode.bytes().chain(grade_band.bytes()) {
        s = s.wrapping_mul(16777619) ^ (b as u64);
    }
    if s == 0 {
        0xCAFE_BABE_1234_5678
    } else {
        s
    }
}

fn shuffle_vec<T>(items: &mut [T], rng: &mut u64) {
    for i in (1..items.len()).rev() {
        let j = (next_rand(rng) as usize) % (i + 1);
        items.swap(i, j);
    }
}

// ── 1. PROCEDURAL CROSSWORD GENERATOR (TEKA-TEKI SILANG) ───────────────────

#[derive(Clone, Copy)]
struct WordEntry {
    word: &'static str,
    clue: &'static str,
    topic: &'static str,
}

const CROSSWORD_LEXICON: &[WordEntry] = &[
    // IPA & Sains
    WordEntry { word: "ATOM", clue: "Partikel terkecil penyusun suatu unsur kimia", topic: "IPA Kimia" },
    WordEntry { word: "OZON", clue: "Lapisan atmosfer pelindung bumi dari sinar UV matahari", topic: "IPA" },
    WordEntry { word: "SEL", clue: "Unit struktural dan fungsional terkecil makhluk hidup", topic: "Biologi" },
    WordEntry { word: "VENA", clue: "Pembuluh darah yang mengalirkan darah kembali ke jantung", topic: "Biologi" },
    WordEntry { word: "AKAR", clue: "Bagian tumbuhan penyerap air dan mineral dari dalam tanah", topic: "Biologi" },
    WordEntry { word: "DAUN", clue: "Organ utama tempat berlangsungnya fotosintesis tumbuhan", topic: "Biologi" },
    WordEntry { word: "MARS", clue: "Planet keempat dari matahari yang dijuluki Planet Merah", topic: "Astronomi" },
    WordEntry { word: "BUMI", clue: "Planet ketiga dalam tata surya tempat manusia hidup", topic: "Astronomi" },
    WordEntry { word: "ORBIT", clue: "Lintasan melengkung planet saat mengelilingi matahari", topic: "Astronomi" },
    WordEntry { word: "KARBON", clue: "Unsur utama penyusun senyawa organik, arang, dan intan", topic: "Kimia" },
    WordEntry { word: "OKSIGEN", clue: "Gas di udara yang dibutuhkan manusia untuk bernapas", topic: "IPA" },
    WordEntry { word: "PROTON", clue: "Partikel bermuatan listrik positif di dalam inti atom", topic: "Fisika" },
    WordEntry { word: "MASSA", clue: "Besaran pokok fisika yang diukur dengan satuan kilogram", topic: "Fisika" },
    WordEntry { word: "MAGNET", clue: "Benda yang memiliki kutub utara dan selatan serta menarik besi", topic: "Fisika" },
    WordEntry { word: "ENZYM", clue: "Protein biologis yang mempercepat reaksi kimia di tubuh", topic: "Biologi" },
    WordEntry { word: "LENSA", clue: "Kaca bening pembias cahaya pada kacamata atau mikroskop", topic: "Fisika" },
    WordEntry { word: "KLOROFIL", clue: "Zat hijau daun yang menangkap energi cahaya matahari", topic: "Biologi" },
    WordEntry { word: "STOMATA", clue: "Pori-pori mikroskopis di daun tempat keluar masuknya gas", topic: "Biologi" },
    WordEntry { word: "NEUTRON", clue: "Partikel inti atom yang tidak memiliki muatan listrik", topic: "Fisika" },
    // Matematika & Logika
    WordEntry { word: "PRIMA", clue: "Bilangan bulat lebih dari 1 yang hanya habis dibagi 1 dan dirinya sendiri", topic: "Matematika" },
    WordEntry { word: "SUDUT", clue: "Daerah yang dibentuk oleh dua garis lurus yang berpotongan di satu titik", topic: "Geometri" },
    WordEntry { word: "KUBUS", clue: "Bangun ruang dengan enam sisi berbentuk persegi sama luas", topic: "Geometri" },
    WordEntry { word: "LIMAS", clue: "Bangun ruang bersisi tegak segitiga dan berpuncak lancip", topic: "Geometri" },
    WordEntry { word: "RADIUS", clue: "Jarak dari titik pusat ke tepi lingkaran (jari-jari)", topic: "Matematika" },
    WordEntry { word: "MEAN", clue: "Nilai rata-rata dari sekumpulan data angka", topic: "Statistika" },
    WordEntry { word: "MODUS", clue: "Nilai yang paling sering muncul dalam sebuah data", topic: "Statistika" },
    WordEntry { word: "PERSEN", clue: "Perbandingan atau pecahan yang dinyatakan per seratus", topic: "Matematika" },
    WordEntry { word: "GRAFIK", clue: "Penyajian visual data hubungan antar-angka dalam diagram", topic: "Matematika" },
    // IPS, Geografi & Sejarah
    WordEntry { word: "PETA", clue: "Gambaran permukaan bumi pada bidang datar dengan skala tertentu", topic: "Geografi" },
    WordEntry { word: "ATLAS", clue: "Kumpulan peta bumi yang dibukukan beserta indeks wilayahnya", topic: "Geografi" },
    WordEntry { word: "DELTA", clue: "Endapan tanah berbentuk kipas di muara sungai dekat laut", topic: "Geografi" },
    WordEntry { word: "BARTER", clue: "Sistem tukar-menukar barang langsung sebelum uang ditemukan", topic: "Ekonomi" },
    WordEntry { word: "LABA", clue: "Keuntungan bersih selisih antara pendapatan dan biaya modal", topic: "Ekonomi" },
    WordEntry { word: "INFLASI", clue: "Kenaikan harga barang dan jasa secara umum dan terus menerus", topic: "Ekonomi" },
    WordEntry { word: "PAJAK", clue: "Iuran wajib warga negara kepada kas negara berdasarkan undang-undang", topic: "Ekonomi" },
    WordEntry { word: "EQUATOR", clue: "Garis khayal khatulistiwa yang membelah bumi utara dan selatan", topic: "Geografi" },
    WordEntry { word: "SUMATERA", clue: "Pulau besar di bagian barat Indonesia penghubung Selat Malaka", topic: "Geografi" },
    WordEntry { word: "CANDI", clue: "Bangunan batu peninggalan masa sejarah kerajaan Hindu-Buddha", topic: "Sejarah" },
    WordEntry { word: "SUMPAH", clue: "Ikrar Pemuda 28 Oktober 1928: ... Pemuda Indonesia", topic: "Sejarah" },
    WordEntry { word: "GARUDA", clue: "Burung lambang negara Republik Indonesia", topic: "PPKn" },
    WordEntry { word: "NORMA", clue: "Aturan atau pedoman perilaku yang berlaku dalam masyarakat", topic: "PPKn" },
    WordEntry { word: "DEMOKRASI", clue: "Sistem pemerintahan dari rakyat, oleh rakyat, dan untuk rakyat", topic: "PPKn" },
    // Bahasa Indonesia & Literasi
    WordEntry { word: "DIKSI", clue: "Pilihan kata yang tepat dan selaras untuk mengungkapkan gagasan", topic: "Bahasa" },
    WordEntry { word: "MAJAS", clue: "Gaya bahasa kiasan untuk memperindah makna kalimat", topic: "Bahasa" },
    WordEntry { word: "PANTUN", clue: "Puisi lama empat baris bersajak a-b-a-b dengan sampiran dan isi", topic: "Sastra" },
    WordEntry { word: "SINONIM", clue: "Dua kata berbeda bentuk namun memiliki arti yang sama atau mirip", topic: "Bahasa" },
    WordEntry { word: "ANTONIM", clue: "Pasangan kata yang memiliki makna saling berlawanan", topic: "Bahasa" },
    WordEntry { word: "SAJAK", clue: "Persamaan bunyi huruf atau suku kata pada akhir baris puisi", topic: "Sastra" },
    WordEntry { word: "DRAMA", clue: "Karya sastra berupa dialog yang dipentaskan di atas panggung", topic: "Sastra" },
    WordEntry { word: "KALIMAT", clue: "Susunan kata bermakna lengkap yang minimal berpola subjek-predikat", topic: "Bahasa" },
    // Informatika & Digital
    WordEntry { word: "DATA", clue: "Fakta mentah berupa angka, teks, atau gambar untuk diolah komputer", topic: "Informatika" },
    WordEntry { word: "KODE", clue: "Baris instruksi bahasa pemrograman yang dijalankan oleh komputer", topic: "Informatika" },
    WordEntry { word: "PIXEL", clue: "Titik warna terkecil penyusun gambar digital di layar", topic: "Informatika" },
    WordEntry { word: "SERVER", clue: "Komputer penyedia layanan data yang diakses klien lewat jaringan", topic: "Informatika" },
    WordEntry { word: "ROUTER", clue: "Perangkat pengatur lalu lintas paket data di jaringan Wi-Fi/internet", topic: "Informatika" },
    WordEntry { word: "BINARY", clue: "Sistem bilangan komputer yang hanya memakai angka 0 dan 1", topic: "Informatika" },
    WordEntry { word: "SANDI", clue: "Kata rahasia atau password untuk menjaga keamanan akun digital", topic: "Literasi Digital" },
    WordEntry { word: "ROBOT", clue: "Mesin otomatis terprogram yang dapat melakukan tugas fisik atau digital", topic: "Teknologi" },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Across,
    Down,
}

#[derive(Clone)]
struct PlacedWord {
    entry: WordEntry,
    row: usize,
    col: usize,
    dir: Dir,
}

fn can_place_word(
    grid: &[[char; 11]; 11],
    word: &str,
    sr: isize,
    sc: isize,
    dir: Dir,
) -> Option<usize> {
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len() as isize;
    let (dr, dc) = if dir == Dir::Across { (0, 1) } else { (1, 0) };

    let er = sr + dr * (len - 1);
    let ec = sc + dc * (len - 1);
    if sr < 0 || sc < 0 || er >= 11 || ec >= 11 {
        return None;
    }

    // Cell right before word & right after word must be empty
    let br = sr - dr;
    let bc = sc - dc;
    if br >= 0 && bc >= 0 && grid[br as usize][bc as usize] != '\0' {
        return None;
    }
    let ar = er + dr;
    let ac = ec + dc;
    if ar < 11 && ac < 11 && grid[ar as usize][ac as usize] != '\0' {
        return None;
    }

    let mut intersections = 0usize;
    for (k, &ch) in chars.iter().enumerate() {
        let r = (sr + dr * (k as isize)) as usize;
        let c = (sc + dc * (k as isize)) as usize;
        let existing = grid[r][c];
        if existing == '\0' {
            // Check perpendicular neighbors so parallel words don't smear together
            let (pr1, pc1) = (r as isize + dc, c as isize + dr);
            let (pr2, pc2) = (r as isize - dc, c as isize - dr);
            if pr1 >= 0 && pr1 < 11 && pc1 >= 0 && pc1 < 11 && grid[pr1 as usize][pc1 as usize] != '\0' {
                return None;
            }
            if pr2 >= 0 && pr2 < 11 && pc2 >= 0 && pc2 < 11 && grid[pr2 as usize][pc2 as usize] != '\0' {
                return None;
            }
        } else if existing == ch {
            // Ensure no collinear continuation on existing letter
            let prev_collinear = if k == 0 {
                false
            } else {
                let pr = (sr + dr * (k as isize - 1)) as usize;
                let pc = (sc + dc * (k as isize - 1)) as usize;
                grid[pr][pc] != '\0'
            };
            if prev_collinear {
                return None;
            }
            intersections += 1;
        } else {
            return None;
        }
    }

    if intersections > 0 {
        Some(intersections)
    } else {
        None
    }
}

fn commit_word(grid: &mut [[char; 11]; 11], word: &str, row: usize, col: usize, dir: Dir) {
    let (dr, dc) = if dir == Dir::Across { (0, 1) } else { (1, 0) };
    for (k, ch) in word.chars().enumerate() {
        grid[row + dr * k][col + dc * k] = ch;
    }
}

fn generate_crossword_challenge(rng: &mut u64) -> Value {
    let mut best_placed: Vec<PlacedWord> = Vec::new();
    let mut best_grid = [['\0'; 11]; 11];

    for _attempt in 0..8 {
        let mut pool: Vec<WordEntry> = CROSSWORD_LEXICON.to_vec();
        shuffle_vec(&mut pool, rng);

        // Pick a strong seed word of 5..8 chars
        let anchor_idx = pool
            .iter()
            .position(|e| (5..=8).contains(&e.word.chars().count()))
            .unwrap_or(0);
        let anchor = pool.remove(anchor_idx);
        let anchor_len = anchor.word.chars().count();

        let mut grid = [['\0'; 11]; 11];
        let mut placed: Vec<PlacedWord> = Vec::new();
        let start_col = (11usize.saturating_sub(anchor_len)) / 2;
        commit_word(&mut grid, anchor.word, 5, start_col, Dir::Across);
        placed.push(PlacedWord {
            entry: anchor,
            row: 5,
            col: start_col,
            dir: Dir::Across,
        });

        let mut used_words: Vec<&'static str> = vec![anchor.word];

        for _pass in 0..2 {
            for cand in &pool {
                if placed.len() >= 7 {
                    break;
                }
                if used_words.contains(&cand.word) {
                    continue;
                }
                let cchars: Vec<char> = cand.word.chars().collect();
                let mut best_fit: Option<(usize, usize, usize, Dir)> = None;

                for pw in &placed {
                    let target_dir = if pw.dir == Dir::Across { Dir::Down } else { Dir::Across };
                    let (pdr, pdc) = if pw.dir == Dir::Across { (0, 1) } else { (1, 0) };
                    for (pk, pch) in pw.entry.word.chars().enumerate() {
                        let ir = pw.row + pdr * pk;
                        let ic = pw.col + pdc * pk;
                        for (ck, &cch) in cchars.iter().enumerate() {
                            if cch != pch {
                                continue;
                            }
                            let (sr, sc) = if target_dir == Dir::Across {
                                (ir as isize, ic as isize - ck as isize)
                            } else {
                                (ir as isize - ck as isize, ic as isize)
                            };
                            if let Some(inter) = can_place_word(&grid, cand.word, sr, sc, target_dir) {
                                let center_dist =
                                    ((sr - 5).abs() + (sc - 5).abs()) as usize;
                                let score = inter * 20 + 20usize.saturating_sub(center_dist);
                                let replace = match best_fit {
                                    None => true,
                                    Some((bscore, _, _, _)) => score > bscore,
                                };
                                if replace {
                                    best_fit = Some((score, sr as usize, sc as usize, target_dir));
                                }
                            }
                        }
                    }
                }

                if let Some((_, r, c, dir)) = best_fit {
                    commit_word(&mut grid, cand.word, r, c, dir);
                    placed.push(PlacedWord {
                        entry: *cand,
                        row: r,
                        col: c,
                        dir,
                    });
                    used_words.push(cand.word);
                }
            }
        }

        if placed.len() > best_placed.len() {
            best_placed = placed;
            best_grid = grid;
        }
        if best_placed.len() >= 6 {
            break;
        }
    }

    // Trim grid bounding box
    let mut min_r = 10usize;
    let mut max_r = 0usize;
    let mut min_c = 10usize;
    let mut max_c = 0usize;
    for r in 0..11 {
        for c in 0..11 {
            if best_grid[r][c] != '\0' {
                min_r = min_r.min(r);
                max_r = max_r.max(r);
                min_c = min_c.min(c);
                max_c = max_c.max(c);
            }
        }
    }

    let rows = max_r.saturating_sub(min_r) + 1;
    let cols = max_c.saturating_sub(min_c) + 1;
    let mut compact_grid: Vec<String> = Vec::with_capacity(rows);
    for r in min_r..=max_r {
        let mut row_str = String::with_capacity(cols);
        for c in min_c..=max_c {
            let ch = best_grid[r][c];
            row_str.push(if ch == '\0' { '#' } else { ch });
        }
        compact_grid.push(row_str);
    }

    // Assign clue numbers in reading order (row-major)
    let mut starts: Vec<(usize, usize, usize)> = Vec::new();
    for (idx, pw) in best_placed.iter().enumerate() {
        starts.push((pw.row - min_r, pw.col - min_c, idx));
    }
    starts.sort_by_key(|&(r, c, _)| (r, c));

    let mut cell_numbers: std::collections::HashMap<(usize, usize), usize> =
        std::collections::HashMap::new();
    let mut next_num = 1usize;
    for &(r, c, _) in &starts {
        cell_numbers.entry((r, c)).or_insert_with(|| {
            let n = next_num;
            next_num += 1;
            n
        });
    }

    let mut clues_json: Vec<Value> = best_placed
        .iter()
        .map(|pw| {
            let r = pw.row - min_r;
            let c = pw.col - min_c;
            let num = *cell_numbers.get(&(r, c)).unwrap_or(&1);
            json!({
                "number": num,
                "direction": if pw.dir == Dir::Across { "ACROSS" } else { "DOWN" },
                "row": r,
                "col": c,
                "length": pw.entry.word.chars().count(),
                "answer": pw.entry.word,
                "clue": pw.entry.clue,
                "topic_tag": pw.entry.topic
            })
        })
        .collect();

    clues_json.sort_by_key(|v| {
        let dir_rank = if v["direction"].as_str() == Some("ACROSS") {
            0usize
        } else {
            1usize
        };
        let num = v["number"].as_u64().unwrap_or(0) as usize;
        (dir_rank, num)
    });

    json!({
        "id": format!("cw_{}", next_rand(rng) % 100_000),
        "rows": rows,
        "cols": cols,
        "grid": compact_grid,
        "clues": clues_json,
        "topic_tag": "Lintas Pelajaran"
    })
}

// ── 2. PROCEDURAL SLIDING PUZZLE GENERATOR (PUZZLE GESER) ────────────────────

struct PuzzleTheme {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
}

const PUZZLE_THEMES: &[PuzzleTheme] = &[
    PuzzleTheme {
        id: "PET_TREASURE",
        title: "Ciko & Peti Harta Ilmu",
        subtitle: "Susun kepingan ruang belajar ajaib & peti koin Ciko",
    },
    PuzzleTheme {
        id: "NUSANTARA_MAP",
        title: "Peta Eksplorasi Nusantara",
        subtitle: "Rangkai panorama peta budaya & wilayah kepulauan",
    },
    PuzzleTheme {
        id: "MORNING_STUDY",
        title: "Ruang Belajar Pagi",
        subtitle: "Susun kembali kamar belajar yang hangat dan rapi",
    },
    PuzzleTheme {
        id: "STUDENT_QUEST",
        title: "Misi Pelajar Cerdas",
        subtitle: "Satukan potongan ilustrasi aktivitas belajar seru",
    },
];

fn generate_sliding_puzzle_challenge(rng: &mut u64, grid_size: usize) -> Value {
    let total = grid_size * grid_size;
    let mut tiles: Vec<usize> = (1..total).collect();
    tiles.push(0); // 0 = empty blank slot

    let mut blank = total - 1;
    let mut prev_blank = blank;
    let shuffle_steps = total * 14;

    for _ in 0..shuffle_steps {
        let br = blank / grid_size;
        let bc = blank % grid_size;
        let mut neighbors: Vec<usize> = Vec::with_capacity(4);
        if br > 0 {
            neighbors.push((br - 1) * grid_size + bc);
        }
        if br + 1 < grid_size {
            neighbors.push((br + 1) * grid_size + bc);
        }
        if bc > 0 {
            neighbors.push(br * grid_size + (bc - 1));
        }
        if bc + 1 < grid_size {
            neighbors.push(br * grid_size + (bc + 1));
        }
        neighbors.retain(|&n| n != prev_blank);
        if !neighbors.is_empty() {
            let pick = neighbors[(next_rand(rng) as usize) % neighbors.len()];
            tiles.swap(blank, pick);
            prev_blank = blank;
            blank = pick;
        }
    }

    // Ensure not accidentally already solved
    let already_solved = tiles
        .iter()
        .enumerate()
        .all(|(i, &val)| if i + 1 == total { val == 0 } else { val == i + 1 });
    if already_solved && total >= 4 {
        tiles.swap(total - 1, total - 2);
        tiles.swap(0, 1);
    }

    let theme = &PUZZLE_THEMES[(next_rand(rng) as usize) % PUZZLE_THEMES.len()];
    let seed = next_rand(rng) % 9_999_999;

    json!({
        "id": format!("sp_{}", seed),
        "grid_size": grid_size,
        "theme_id": theme.id,
        "theme_title": theme.title,
        "theme_subtitle": theme.subtitle,
        "initial_tiles": tiles,
        "par_moves": if grid_size == 3 { 35 } else { 85 },
        "seed": seed
    })
}

// ── 3. PROCEDURAL BLOCK BLAST CHALLENGE ────────────────────────────────────

fn generate_block_puzzle_challenge(rng: &mut u64, round: i32) -> Value {
    let seed = (next_rand(rng) % 9_000_000) + 1_000_000;
    let target_score = 350 + (round.clamp(0, 5) * 120);
    let target_lines = 4 + round.clamp(0, 4);

    // Optional light starter mosaic on higher rounds to make every board unique
    let mut prefilled: Vec<[usize; 2]> = Vec::new();
    if round > 0 {
        let count = 3 + ((next_rand(rng) as usize) % 4);
        for _ in 0..count {
            let r = (next_rand(rng) as usize) % 8;
            let c = (next_rand(rng) as usize) % 8;
            let pair = [r, c];
            if !prefilled.contains(&pair) {
                prefilled.push(pair);
            }
        }
    }

    json!({
        "id": format!("bp_{}", seed),
        "board_size": 8,
        "target_score": target_score,
        "target_lines": target_lines,
        "seed": seed,
        "prefilled_cells": prefilled,
        "theme_title": if round > 0 { format!("Misi Balok Level #{}", round + 1) } else { "Arena Block Blast Harian".to_string() }
    })
}

// ── 4. PROCEDURAL 2048 ANGKA CHALLENGE ─────────────────────────────────────

fn generate_number_merge_challenge(rng: &mut u64, round: i32) -> Value {
    let seed = (next_rand(rng) % 9_000_000) + 1_000_000;
    let target_tile = match round {
        0 => 256,
        1 => 512,
        _ => 1024,
    };
    let target_score = target_tile * 6;

    // Seed two distinct initial grid slots [r, c, value]
    let p1 = (next_rand(rng) as usize) % 16;
    let mut p2 = (next_rand(rng) as usize) % 16;
    while p2 == p1 {
        p2 = (next_rand(rng) as usize) % 16;
    }
    let v1 = if next_rand(rng) % 5 == 0 { 4 } else { 2 };
    let v2 = if next_rand(rng) % 6 == 0 { 4 } else { 2 };

    json!({
        "id": format!("nm_{}", seed),
        "board_size": 4,
        "target_tile": target_tile,
        "target_score": target_score,
        "seed": seed,
        "initial_tiles": [
            { "row": p1 / 4, "col": p1 % 4, "value": v1 },
            { "row": p2 / 4, "col": p2 % 4, "value": v2 }
        ],
        "theme_title": format!("Gabung Angka Menuju {}", target_tile)
    })
}

fn build_game_payload(
    mode: &str,
    round: i32,
    rng: &mut u64,
) -> (String, String, String, Value) {
    match mode {
        "CROSSWORD" => {
            let cw = generate_crossword_challenge(rng);
            (
                "Teka-Teki Silang (TTS)".to_string(),
                "Isi kotak mendatar dan menurun berdasarkan petunjuk wawasan!".to_string(),
                "Bahasa & Wawasan".to_string(),
                json!([cw]),
            )
        }
        "SLIDING_PUZZLE" => {
            let sp = generate_sliding_puzzle_challenge(rng, 3);
            let title = sp["theme_title"]
                .as_str()
                .unwrap_or("Puzzle Geser Visual")
                .to_string();
            (
                format!("Puzzle Geser: {}", title),
                "Geser ubin ke ruang kosong hingga membentuk gambar yang utuh!".to_string(),
                "Visual & Logika Spasial".to_string(),
                json!([sp]),
            )
        }
        "BLOCK_PUZZLE" => {
            let bp = generate_block_puzzle_challenge(rng, round);
            (
                "Block Blast 8×8".to_string(),
                "Susun 3 balok warna ke papan 8×8, bersihkan baris & kolom penuh!".to_string(),
                "Strategi Geometri".to_string(),
                json!([bp]),
            )
        }
        _ => {
            let nm = generate_number_merge_challenge(rng, round);
            (
                "2048 Gabung Angka".to_string(),
                "Geser papan untuk menggabungkan angka kembar hingga target harian!".to_string(),
                "Logika & Numerasi".to_string(),
                json!([nm]),
            )
        }
    }
}

async fn award_extra_xp(
    pool: &PgPool,
    tenant_id: Uuid,
    student_id: Uuid,
    action_type: &str,
    xp_amount: i32,
    desc: &str,
) -> (i64, i32, i32, bool) {
    let today = wib_today();
    let _ = sqlx::query(
        r#"
        INSERT INTO student_xp_transactions (id, tenant_id, student_id, action_type, reference_id, xp_amount, description, created_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (student_id, action_type, reference_id) DO NOTHING
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .bind(action_type)
    .bind(Uuid::new_v4())
    .bind(xp_amount)
    .bind(desc)
    .execute(pool)
    .await;

    let current_xp_row = sqlx::query(
        "SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1",
    )
    .bind(student_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (old_total, old_level, old_streak, last_date) = match current_xp_row {
        Some(r) => (
            r.get::<i64, _>("total_xp"),
            r.get::<i32, _>("level"),
            r.get::<i32, _>("streak_days"),
            r.try_get::<Option<NaiveDate>, _>("last_activity_date")
                .ok()
                .flatten(),
        ),
        None => (0, 1, 0, None),
    };

    let new_total = old_total + (xp_amount as i64);
    let new_level = calculate_level(new_total);
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
    .bind(tenant_id)
    .bind(new_total)
    .bind(new_level)
    .bind(new_streak)
    .bind(today)
    .execute(pool)
    .await;

    (new_total, new_level, new_streak, new_level > old_level)
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
    let round = query.round.max(0);
    let featured_mode = featured_mode_for_date(today);
    let requested_mode = normalize_mode(query.mode.as_deref(), featured_mode);

    let student_row = sqlx::query(
        r#"
        SELECT s.id, s.full_name, COALESCE(c.name, 'Umum') AS class_name
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

    let (student_id_opt, class_name) = match student_row {
        Some(r) => (
            Some(r.get::<Uuid, _>("id")),
            r.try_get::<String, _>("class_name")
                .unwrap_or_else(|_| "Umum".to_string()),
        ),
        None => (None, "Umum".to_string()),
    };

    let grade_band = detect_grade_band(&class_name);
    let mut rng = seed_for_quest(today, round, actor.id, &requested_mode, &grade_band);

    let existing = if round == 0 {
        sqlx::query(
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
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?
    } else {
        None
    };

    let (quest_id, title, subtitle, subject_name, source_label, xp_reward, challenges) =
        match existing {
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
                let (gen_title, gen_subtitle, gen_subject, gen_challenges) =
                    build_game_payload(&requested_mode, round, &mut rng);
                let base_xp = if round > 0 {
                    25
                } else if requested_mode == featured_mode {
                    80
                } else {
                    60
                };
                let src = "PROCEDURAL_ENGINE".to_string();

                if round == 0 {
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
                    .bind(&src)
                    .bind(base_xp)
                    .bind(&gen_challenges)
                    .fetch_one(&ctx.pool)
                    .await
                    .map_err(|e| {
                        ApiError::new(
                            ApplicationError::Infrastructure(InfrastructureError::Database(e)),
                            &req_ctx.request_id,
                        )
                    })?;

                    (
                        row.get::<Uuid, _>("id"),
                        row.get::<String, _>("title"),
                        row.get::<String, _>("subtitle"),
                        row.get::<String, _>("subject_name"),
                        row.get::<String, _>("source_label"),
                        row.get::<i32, _>("xp_reward"),
                        row.get::<Value, _>("challenges_json"),
                    )
                } else {
                    (
                        Uuid::new_v4(),
                        format!("{} (Level #{})", gen_title, round + 1),
                        gen_subtitle,
                        gen_subject,
                        src,
                        base_xp,
                        gen_challenges,
                    )
                }
            }
        };

    let mut my_attempt: Option<DailyQuestAttemptSummary> = None;
    let mut completed_modes_today: Vec<String> = Vec::new();
    let mut total_xp: i64 = 0;
    let mut level: i32 = 1;
    let mut streak_days: i32 = 1;
    let mut chest_claimed_today = false;

    let pet_status = if let Some(sid) = student_id_opt {
        if round == 0 {
            if let Ok(Some(att)) = sqlx::query(
                "SELECT score_percent, correct_count, total_count, max_combo, duration_seconds, xp_earned FROM daily_quest_attempts WHERE quest_id = $1 AND student_id = $2 LIMIT 1",
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
            completed_modes_today = rows
                .into_iter()
                .map(|r| r.get::<String, _>("game_mode"))
                .collect();
        }

        if let Ok(Some(xp_row)) = sqlx::query(
            "SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2 LIMIT 1",
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

        chest_claimed_today = sqlx::query(
            "SELECT 1 FROM daily_quest_chest_claims WHERE student_id = $1 AND claim_date = $2 LIMIT 1",
        )
        .bind(sid)
        .bind(today)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .is_some();

        Some(load_or_create_pet(&ctx.pool, req_ctx.tenant_id, sid).await)
    } else {
        Some(build_pet_status(1, 25, 0, 2, 1))
    };

    let lb_rows = if round == 0 {
        sqlx::query(
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
        .unwrap_or_default()
    } else {
        Vec::new()
    };

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

    let available_modes = ALL_MODES
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
            round_number: round,
            challenges,
            already_completed: my_attempt.is_some(),
            my_attempt,
            streak_days,
            total_xp,
            level,
            available_modes,
            leaderboard,
            pet_status,
            chest_claimed_today,
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
    let today = wib_today();

    let quest_row = sqlx::query(
        "SELECT id, title, xp_reward FROM daily_quests WHERE id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(payload.quest_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let is_bonus_ephemeral = quest_row.is_none() || payload.round_number > 0;
    let (quest_title, base_reward) = match quest_row {
        Some(r) => (r.get::<String, _>("title"), r.get::<i32, _>("xp_reward")),
        None => ("Level Lanjutan Zona Game".to_string(), 25),
    };

    let safe_total = payload.total_count.max(1);
    let safe_correct = payload.correct_count.clamp(0, safe_total);
    let score_percent = ((safe_correct as f64 / safe_total as f64) * 100.0).round() as i32;
    let combo_bonus = (payload.max_combo.max(0) * 5).min(25);
    let completion_bonus = (base_reward / 3).max(20);
    let accuracy_xp =
        ((base_reward as f64 * (safe_correct as f64 / safe_total as f64)).round() as i32).max(10);
    let computed_xp = if is_bonus_ephemeral {
        20
    } else {
        (completion_bonus + accuracy_xp + combo_bonus).clamp(25, 140)
    };

    let student_opt = sqlx::query(
        "SELECT id FROM students WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1",
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
                biscuits_earned: if safe_correct > 0 { 2 } else { 0 },
                pet_status: Some(build_pet_status(1, 25, 0, 2, 1)),
                message: format!(
                    "Mode Pratinjau selesai dengan skor {}% (simulasi +{} XP).",
                    score_percent, computed_xp
                ),
            },
            req_ctx.request_id,
        )));
    };

    let student_id: Uuid = st_row.get("id");
    let _ = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;

    if is_bonus_ephemeral {
        let bonus_row = sqlx::query(
            "SELECT COUNT(*)::int AS cnt, COALESCE(SUM(biscuits_earned), 0)::int AS bis FROM daily_quest_bonus_logs WHERE student_id = $1 AND bonus_date = $2",
        )
        .bind(student_id)
        .bind(today)
        .fetch_one(&ctx.pool)
        .await;

        let (bonus_cnt, bonus_bis_today): (i32, i32) = match bonus_row {
            Ok(r) => (r.try_get("cnt").unwrap_or(0), r.try_get("bis").unwrap_or(0)),
            Err(_) => (0, 0),
        };

        let biscuits_earned = if safe_correct > 0 && bonus_bis_today < 8 {
            1
        } else {
            0
        };
        let xp_bonus = if safe_correct > 0 && bonus_cnt < 5 {
            20
        } else {
            0
        };

        if biscuits_earned > 0 || xp_bonus > 0 {
            let _ = sqlx::query(
                "INSERT INTO daily_quest_bonus_logs (id, tenant_id, student_id, bonus_date, biscuits_earned, created_at) VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())",
            )
            .bind(req_ctx.tenant_id)
            .bind(student_id)
            .bind(today)
            .bind(biscuits_earned)
            .execute(&ctx.pool)
            .await;

            if biscuits_earned > 0 {
                let _ = sqlx::query(
                    "UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1",
                )
                .bind(student_id)
                .bind(biscuits_earned)
                .execute(&ctx.pool)
                .await;
            }
        }

        let (total_xp, level, streak_days, leveled_up) = if xp_bonus > 0 {
            award_extra_xp(
                &ctx.pool,
                req_ctx.tenant_id,
                student_id,
                "DAILY_QUEST_BONUS",
                xp_bonus,
                &format!("Level Lanjutan Game ({}%)", score_percent),
            )
            .await
        } else {
            let cur = sqlx::query(
                "SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2",
            )
            .bind(student_id)
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
            match cur {
                Some(r) => (r.get("total_xp"), r.get("level"), r.get("streak_days"), false),
                None => (0, 1, 1, false),
            }
        };

        let pet_status = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
        let msg = if xp_bonus > 0 || biscuits_earned > 0 {
            format!(
                "Level Baru selesai ({}%)! Raih +{} XP & +{} Koin Energi!",
                score_percent, xp_bonus, biscuits_earned
            )
        } else {
            format!(
                "Latihan selesai ({}%)! Terus cetak skor terbaikmu!",
                score_percent
            )
        };

        return Ok(Json(ApiResponse::success(
            SubmitDailyQuestResponse {
                awarded: xp_bonus > 0 || biscuits_earned > 0,
                already_completed: false,
                xp_earned: xp_bonus,
                score_percent,
                total_xp,
                level,
                streak_days,
                leveled_up,
                is_preview_account: false,
                biscuits_earned,
                pet_status: Some(pet_status),
                message: msg,
            },
            req_ctx.request_id,
        )));
    }

    let existing_attempt = sqlx::query(
        "SELECT xp_earned, score_percent FROM daily_quest_attempts WHERE quest_id = $1 AND student_id = $2 LIMIT 1",
    )
    .bind(payload.quest_id)
    .bind(student_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    if existing_attempt.is_some() {
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

        let cur = sqlx::query(
            "SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2",
        )
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
        let pet_status = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;

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
                biscuits_earned: 0,
                pet_status: Some(pet_status),
                message: format!(
                    "Latihan selesai (skor {}%)! Tekan 'Level Baru' untuk papan baru + bonus XP.",
                    score_percent
                ),
            },
            req_ctx.request_id,
        )));
    }

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

    let biscuits_earned = 2;
    let _ = sqlx::query(
        "UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1",
    )
    .bind(student_id)
    .bind(biscuits_earned)
    .execute(&ctx.pool)
    .await;

    let tx_desc = format!("Zona Game: {} ({}%)", quest_title, score_percent);
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
        "SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1",
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
            r.try_get::<Option<NaiveDate>, _>("last_activity_date")
                .ok()
                .flatten(),
        ),
        None => (0, 1, 0, None),
    };

    let new_total = old_total + (computed_xp as i64);
    let new_level = calculate_level(new_total);
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

    let pet_status = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
    let leveled_up = new_level > old_level;
    let msg = if leveled_up {
        format!(
            "Hebat! Naik Level {} & raih +{} XP + 2 Koin Energi!",
            new_level, computed_xp
        )
    } else {
        format!(
            "Game tuntas ({}%)! +{} XP & +2 Koin Energi!",
            score_percent, computed_xp
        )
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
            biscuits_earned,
            pet_status: Some(pet_status),
            message: msg,
        },
        req_ctx.request_id,
    )))
}

pub async fn feed_shiba_pet(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<FeedPetResponse>>, ApiError> {
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

    let student_opt = sqlx::query(
        "SELECT id FROM students WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(actor.id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let Some(st_row) = student_opt else {
        return Ok(Json(ApiResponse::success(
            FeedPetResponse {
                fed: false,
                evolved: false,
                bonus_xp_earned: 0,
                total_xp: 0,
                level: 1,
                pet_status: build_pet_status(1, 50, 1, 1, 1),
                message: "Mode Pratinjau: Ciko senang menerima koin energimu!".to_string(),
            },
            req_ctx.request_id,
        )));
    };

    let student_id: Uuid = st_row.get("id");
    let pet = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
    if pet.biscuits <= 0 {
        let cur = sqlx::query("SELECT total_xp, level FROM student_xp WHERE student_id = $1 LIMIT 1")
            .bind(student_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
        let (txp, lvl): (i64, i32) = cur
            .map(|r| (r.get("total_xp"), r.get("level")))
            .unwrap_or((0, 1));
        return Ok(Json(ApiResponse::success(
            FeedPetResponse {
                fed: false,
                evolved: false,
                bonus_xp_earned: 0,
                total_xp: txp,
                level: lvl,
                pet_status: pet,
                message: "Koin energi habis! Selesaikan salah satu game untuk dapat koin baru."
                    .to_string(),
            },
            req_ctx.request_id,
        )));
    }

    let next_points = pet.food_points + 25;
    let evolved = next_points >= 100;
    let final_stage = if evolved {
        (pet.pet_stage + 1).min(4)
    } else {
        pet.pet_stage
    };
    let final_points = if evolved {
        next_points - 100
    } else {
        next_points
    };
    let next_tokens = if evolved {
        pet.fifty_fifty_tokens + 1
    } else {
        pet.fifty_fifty_tokens
    };

    let _ = sqlx::query(
        r#"
        UPDATE student_shiba_pets
        SET
            pet_stage = $2,
            food_points = $3,
            total_fed = total_fed + 1,
            biscuits = GREATEST(biscuits - 1, 0),
            fifty_fifty_tokens = $4,
            updated_at = NOW()
        WHERE student_id = $1
        "#,
    )
    .bind(student_id)
    .bind(final_stage)
    .bind(final_points)
    .bind(next_tokens)
    .execute(&ctx.pool)
    .await;

    let bonus_xp = if evolved { 30 } else { 5 };
    let (total_xp, level, _, _) = award_extra_xp(
        &ctx.pool,
        req_ctx.tenant_id,
        student_id,
        "SHIBA_PET_CARE",
        bonus_xp,
        if evolved {
            "Evolusi Maskot Game!"
        } else {
            "Merawat Maskot Game"
        },
    )
    .await;

    let updated_pet = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
    let msg = if evolved {
        format!(
            "YEAY! Ciko naik ke {}! Bonus +{} XP & +1 Petunjuk!",
            updated_pet.stage_title, bonus_xp
        )
    } else {
        format!(
            "Mantap! +{} XP untuk Ciko. Energi: {}/100.",
            bonus_xp, updated_pet.food_points
        )
    };

    Ok(Json(ApiResponse::success(
        FeedPetResponse {
            fed: true,
            evolved,
            bonus_xp_earned: bonus_xp,
            total_xp,
            level,
            pet_status: updated_pet,
            message: msg,
        },
        req_ctx.request_id,
    )))
}

pub async fn open_daily_chest(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<DailyChestResponse>>, ApiError> {
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

    let student_opt = sqlx::query(
        "SELECT id FROM students WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(actor.id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let Some(st_row) = student_opt else {
        return Ok(Json(ApiResponse::success(
            DailyChestResponse {
                opened: true,
                already_claimed: false,
                reward_type: "XP".to_string(),
                reward_amount: 35,
                reward_label: "+35 XP Kejutan".to_string(),
                total_xp: 35,
                level: 1,
                pet_status: build_pet_status(1, 25, 0, 2, 1),
                message: "Mode Pratinjau: Peti Kejutan berisi +35 XP!".to_string(),
            },
            req_ctx.request_id,
        )));
    };

    let student_id: Uuid = st_row.get("id");
    let _ = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;

    let claimed_row = sqlx::query(
        "SELECT reward_type, reward_amount, reward_label FROM daily_quest_chest_claims WHERE student_id = $1 AND claim_date = $2 LIMIT 1",
    )
    .bind(student_id)
    .bind(today)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    if let Some(cr) = claimed_row {
        let cur = sqlx::query("SELECT total_xp, level FROM student_xp WHERE student_id = $1 LIMIT 1")
            .bind(student_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
        let (txp, lvl): (i64, i32) = cur
            .map(|r| (r.get("total_xp"), r.get("level")))
            .unwrap_or((0, 1));
        let pet = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
        return Ok(Json(ApiResponse::success(
            DailyChestResponse {
                opened: false,
                already_claimed: true,
                reward_type: cr.get("reward_type"),
                reward_amount: cr.get("reward_amount"),
                reward_label: cr.get("reward_label"),
                total_xp: txp,
                level: lvl,
                pet_status: pet,
                message: "Peti Kejutan hari ini sudah dibuka! Kembali besok ya.".to_string(),
            },
            req_ctx.request_id,
        )));
    }

    let mut rng = seed_for_quest(today, 77, student_id, "CHEST", "");
    let roll = next_rand(&mut rng) % 4;
    let (reward_type, reward_amount, reward_label) = match roll {
        0 => ("XP", 35, "+35 XP Kejutan".to_string()),
        1 => ("BISCUIT", 2, "+2 Koin Energi Spesial".to_string()),
        2 => ("POWERUP", 1, "+1 Petunjuk & +1 Koin Energi".to_string()),
        _ => ("XP", 50, "JACKPOT +50 XP Harian!".to_string()),
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO daily_quest_chest_claims (id, tenant_id, student_id, claim_date, reward_type, reward_amount, reward_label, created_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (student_id, claim_date) DO NOTHING
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(student_id)
    .bind(today)
    .bind(reward_type)
    .bind(reward_amount)
    .bind(&reward_label)
    .execute(&ctx.pool)
    .await;

    let (total_xp, level) = if reward_type == "XP" {
        let (t, l, _, _) = award_extra_xp(
            &ctx.pool,
            req_ctx.tenant_id,
            student_id,
            "DAILY_CHEST",
            reward_amount,
            &reward_label,
        )
        .await;
        (t, l)
    } else {
        if reward_type == "BISCUIT" {
            let _ = sqlx::query(
                "UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1",
            )
            .bind(student_id)
            .bind(reward_amount)
            .execute(&ctx.pool)
            .await;
        } else {
            let _ = sqlx::query(
                "UPDATE student_shiba_pets SET fifty_fifty_tokens = fifty_fifty_tokens + 1, biscuits = biscuits + 1, updated_at = NOW() WHERE student_id = $1",
            )
            .bind(student_id)
            .execute(&ctx.pool)
            .await;
        }
        let cur = sqlx::query("SELECT total_xp, level FROM student_xp WHERE student_id = $1 LIMIT 1")
            .bind(student_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
        cur.map(|r| (r.get("total_xp"), r.get("level")))
            .unwrap_or((0, 1))
    };

    let pet = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
    Ok(Json(ApiResponse::success(
        DailyChestResponse {
            opened: true,
            already_claimed: false,
            reward_type: reward_type.to_string(),
            reward_amount,
            reward_label: reward_label.clone(),
            total_xp,
            level,
            pet_status: pet,
            message: format!("Peti Kejutan Terbuka: {}!", reward_label),
        },
        req_ctx.request_id,
    )))
}
