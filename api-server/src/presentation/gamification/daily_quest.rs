use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
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
        0 | 5 => "MAP_PIN_HUNT",
        1 => "FACT_OR_MYTH",
        2 | 6 => "SPEED_MATCH",
        3 => "CASE_DETECTIVE",
        4 => "SEQUENCE_ORDER",
        _ => "CONCEPT_GUESS",
    }
}

fn normalize_mode(raw: Option<&str>, fallback: &str) -> String {
    match raw.map(|s| s.trim().to_uppercase()).as_deref() {
        Some("MAP_PIN_HUNT") => "MAP_PIN_HUNT".to_string(),
        Some("SPEED_MATCH") => "SPEED_MATCH".to_string(),
        Some("FACT_OR_MYTH") => "FACT_OR_MYTH".to_string(),
        Some("CASE_DETECTIVE") => "CASE_DETECTIVE".to_string(),
        Some("SEQUENCE_ORDER") => "SEQUENCE_ORDER".to_string(),
        Some("CONCEPT_GUESS") => "CONCEPT_GUESS".to_string(),
        _ => fallback.to_string(),
    }
}

fn mode_display_label(code: &str) -> &'static str {
    match code {
        "MAP_PIN_HUNT" => "Peta & Lokasi",
        "SPEED_MATCH" => "Cocokkan Cepat",
        "FACT_OR_MYTH" => "Geser Fakta/Mitos",
        "CASE_DETECTIVE" => "Komik Kasus",
        "SEQUENCE_ORDER" => "Susun Langkah",
        "CONCEPT_GUESS" => "Tebak Istilah",
        _ => "Misi Harian",
    }
}

fn shiba_stage_title(stage: i32) -> &'static str {
    match stage {
        1 => "Anak Shiba Penasaran",
        2 => "Shiba Pelajar Aktif",
        3 => "Detektif Shiba Pintar",
        _ => "Profesor Shiba Emas",
    }
}

fn build_pet_status(pet_stage: i32, food_points: i32, total_fed: i32, biscuits: i32, tokens: i32) -> ShibaPetStatus {
    let clamped_stage = pet_stage.max(1);
    let points = food_points.clamp(0, 99);
    ShibaPetStatus {
        pet_name: "Shiba Pintar".to_string(),
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
        "SELECT pet_stage, food_points, total_fed, biscuits, fifty_fifty_tokens FROM student_shiba_pets WHERE student_id = $1 LIMIT 1"
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

fn next_rand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn seed_for_quest(date: NaiveDate, round: i32, actor_id: Uuid, mode: &str, grade_band: &str) -> u64 {
    let mut s = ((date.year() as u64) << 16) ^ ((date.ordinal() as u64) << 4) ^ ((round.max(0) as u64).wrapping_mul(2654435761));
    if round > 0 {
        s ^= actor_id.as_u128() as u64;
        s ^= (actor_id.as_u128() >> 64) as u64;
    }
    for b in mode.bytes().chain(grade_band.bytes()) {
        s = s.wrapping_mul(16777619) ^ (b as u64);
    }
    if s == 0 { 0xCAFE_BABE_1234_5678 } else { s }
}

fn shuffle_vec<T>(items: &mut Vec<T>, rng: &mut u64) {
    for i in (1..items.len()).rev() {
        let j = (next_rand(rng) as usize) % (i + 1);
        items.swap(i, j);
    }
}

fn shuffle_case_options(mut item: Value, rng: &mut u64) -> Value {
    let Some(obj) = item.as_object_mut() else {
        return item;
    };
    let Some(opts_val) = obj.get("options").and_then(|v| v.as_array()).cloned() else {
        return item;
    };
    if opts_val.len() < 2 {
        return item;
    }
    let correct_idx = obj.get("correct_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut tagged: Vec<(bool, Value)> = opts_val
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i == correct_idx, v))
        .collect();
    shuffle_vec(&mut tagged, rng);
    let new_correct = tagged.iter().position(|(is_c, _)| *is_c).unwrap_or(0);
    let new_opts: Vec<Value> = tagged.into_iter().map(|(_, v)| v).collect();
    obj.insert("options".to_string(), Value::Array(new_opts));
    obj.insert("correct_index".to_string(), json!(new_correct));
    item
}

fn format_rp(amount: i64) -> String {
    let abs_val = amount.abs();
    let s = abs_val.to_string();
    let mut out = String::new();
    for (idx, ch) in s.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    let formatted: String = out.chars().rev().collect();
    if amount < 0 {
        format!("-Rp{}", formatted)
    } else {
        format!("Rp{}", formatted)
    }
}

fn gen_procedural_sequence_items(rng: &mut u64, count: usize) -> Vec<Value> {
    let mut out = Vec::new();
    for i in 0..count {
        let variant = (next_rand(rng) % 3) as i32;
        if variant == 0 {
            let a = (2 + (next_rand(rng) % 7)) as i32;
            let x = (2 + (next_rand(rng) % 12)) as i32;
            let b = (3 + (next_rand(rng) % 18)) as i32;
            let c = a * x + b;
            let rhs_sub = a * x;
            out.push(json!({
                "id": format!("proc_seq_{}_{}", variant, i),
                "prompt": format!("Susun langkah-langkah penyelesaian persamaan berikut: {}x + {} = {}", a, b, c),
                "steps": [
                    format!("{}x + {} = {}", a, b, c),
                    format!("{}x = {} - {}", a, c, b),
                    format!("{}x = {}", a, rhs_sub),
                    format!("x = {}", x)
                ],
                "explanation": format!("Pindahkan +{} ke kanan menjadi -{} agar {}x = {}, lalu bagi {} sehingga x = {}.", b, b, a, rhs_sub, a, x),
                "topic_tag": "Matematika Aljabar"
            }));
        } else if variant == 1 {
            let a = (2 + (next_rand(rng) % 6)) as i32;
            let x = (3 + (next_rand(rng) % 10)) as i32;
            let b = (2 + (next_rand(rng) % 15)) as i32;
            let c = a * x - b;
            let rhs_sum = a * x;
            out.push(json!({
                "id": format!("proc_seq_{}_{}", variant, i),
                "prompt": format!("Urutkan penyelesaian persamaan linear berikut: {}x - {} = {}", a, b, c),
                "steps": [
                    format!("{}x - {} = {}", a, b, c),
                    format!("{}x = {} + {}", a, c, b),
                    format!("{}x = {}", a, rhs_sum),
                    format!("x = {}", x)
                ],
                "explanation": format!("Pindahkan -{} ke kanan menjadi +{} agar {}x = {}, lalu bagi {} sehingga x = {}.", b, b, a, rhs_sum, a, x),
                "topic_tag": "Aljabar Linear"
            }));
        } else {
            let harga_awal = ((4 + (next_rand(rng) % 6)) * 20_000) as i64;
            let diskon_pct = ((1 + (next_rand(rng) % 4)) * 10) as i64;
            let potongan = harga_awal * diskon_pct / 100;
            let harga_akhir = harga_awal - potongan;
            out.push(json!({
                "id": format!("proc_seq_{}_{}", variant, i),
                "prompt": format!("Urutkan cara menghitung harga bayar tas seharga {} dengan diskon {}%:", format_rp(harga_awal), diskon_pct),
                "steps": [
                    format!("Harga awal = {}", format_rp(harga_awal)),
                    format!("Potongan = {}% x {}", diskon_pct, format_rp(harga_awal)),
                    format!("Potongan = {}", format_rp(potongan)),
                    format!("Bayar = {}", format_rp(harga_akhir))
                ],
                "explanation": format!("Hitung dulu besar diskon {}% dari {} = {}, lalu kurangkan dari harga awal menjadi {}.", diskon_pct, format_rp(harga_awal), format_rp(potongan), format_rp(harga_akhir)),
                "topic_tag": "Numerasi Belanja"
            }));
        }
    }
    out
}

fn gen_procedural_case_items(rng: &mut u64, count: usize) -> Vec<Value> {
    let mut out = Vec::new();
    let names = ["Riko", "Sari", "Bagas", "Nadia", "Damar", "Laras"];
    let items = ["kopi susu", "jus mangga", "es cokelat", "teh tarik", "roti bakar"];
    for i in 0..count {
        let pick = i % 3;
        if pick == 0 {
            let who = names[(next_rand(rng) as usize) % names.len()];
            let prod = items[(next_rand(rng) as usize) % items.len()];
            let qty = (20 + ((next_rand(rng) % 7) * 5)) as i64;
            let sell = (6_000 + ((next_rand(rng) % 5) * 1_000)) as i64;
            let cost = sell - (2_000 + ((next_rand(rng) % 3) * 1_000)) as i64;
            let fixed = (25_000 + ((next_rand(rng) % 6) * 5_000)) as i64;
            let margin_unit = sell - cost;
            let gross_margin = qty * margin_unit;
            let net = gross_margin - fixed;
            let omzet = qty * sell;
            let d1 = gross_margin;
            let d2 = omzet;
            let d3 = net + margin_unit * 5;
            let ch = json!({
                "id": format!("proc_case_umkm_{}", i),
                "scenario": format!("Gerai {} milik {} menjual {} porsi seharga {}/porsi. Biaya bahan {}/porsi, sedangkan biaya listrik & sewa {}/hari.", prod, who, qty, format_rp(sell), format_rp(cost), format_rp(fixed)),
                "question": format!("Berapa keuntungan bersih {} dalam 1 hari?", who),
                "options": [format_rp(net), format_rp(d1), format_rp(d2), format_rp(d3)],
                "correct_index": 0,
                "explanation": format!("Margin per porsi = {} - {} = {}. Total margin {} porsi = {}. Dikurangi biaya tetap {} = {} bersih!", format_rp(sell), format_rp(cost), format_rp(margin_unit), qty, format_rp(gross_margin), format_rp(fixed), format_rp(net)),
                "topic_tag": "Numerasi Usaha"
            });
            out.push(shuffle_case_options(ch, rng));
        } else if pick == 1 {
            let d1 = (20 + (next_rand(rng) % 4) * 10) as i64;
            let d2 = (10 + (next_rand(rng) % 2) * 10) as i64;
            let base = 200_000_i64;
            let after_1 = base - (base * d1 / 100);
            let final_price = after_1 - (after_1 * d2 / 100);
            let wrong_sum = base - (base * (d1 + d2) / 100);
            let wrong_1 = after_1;
            let wrong_2 = base - (base * d2 / 100);
            let ch = json!({
                "id": format!("proc_case_disc_{}", i),
                "scenario": format!("Sebuah toko memasang label 'Diskon {}% + {}%' untuk jaket seharga {}. Kasir menghitung diskon pertama lebih dulu, lalu diskon kedua dari harga sisa.", d1, d2, format_rp(base)),
                "question": "Berapa harga akhir yang harus dibayar di kasir?",
                "options": [format_rp(final_price), format_rp(wrong_sum), format_rp(wrong_1), format_rp(wrong_2)],
                "correct_index": 0,
                "explanation": format!("Setelah diskon {}%, harga menjadi {}. Lalu dipotong {}% lagi dari {} = {}!", d1, format_rp(after_1), d2, format_rp(after_1), format_rp(final_price)),
                "topic_tag": "Numerasi Diskon"
            });
            out.push(shuffle_case_options(ch, rng));
        } else {
            let speed = (40 + (next_rand(rng) % 4) * 10) as i64;
            let hours = (2 + (next_rand(rng) % 4)) as i64;
            let dist = speed * hours;
            let ch = json!({
                "id": format!("proc_case_dist_{}", i),
                "scenario": format!("Bus belajar menempuh perjalanan antarkota dengan kecepatan rata-rata {} km/jam selama {} jam tanpa berhenti.", speed, hours),
                "question": "Berapa total jarak tempuh bus tersebut?",
                "options": [
                    format!("{} km", dist),
                    format!("{} km", dist + speed),
                    format!("{} km", dist - 20),
                    format!("{} km", speed + hours * 10)
                ],
                "correct_index": 0,
                "explanation": format!("Jarak = Kecepatan x Waktu = {} km/jam x {} jam = {} km.", speed, hours, dist),
                "topic_tag": "Numerasi & Fisika"
            });
            out.push(shuffle_case_options(ch, rng));
        }
    }
    out
}

fn gen_procedural_speed_match_items(rng: &mut u64, _grade_band: &str) -> Vec<Value> {
    let mut challenges = Vec::new();

    // Challenge 1: 4 Unique Linear Equations (x values guaranteed distinct!)
    let mut xs: Vec<i32> = (2..=15).collect();
    shuffle_vec(&mut xs, rng);
    let mut eq_pairs = Vec::new();
    for (idx, x) in xs.into_iter().take(4).enumerate() {
        let a = (2 + (next_rand(rng) % 5)) as i32;
        let b = (1 + (next_rand(rng) % 12)) as i32;
        let use_minus = (next_rand(rng) % 2) == 1 && a * x > b;
        let eq_str = if use_minus {
            format!("{}x - {} = {}", a, b, a * x - b)
        } else {
            format!("{}x + {} = {}", a, b, a * x + b)
        };
        eq_pairs.push(json!({
            "id": format!("alg_{}", idx),
            "left": eq_str,
            "right": format!("x = {}", x),
        }));
    }
    challenges.push(json!({
        "id": "sm_algebra_dyn",
        "prompt": "Pasangkan setiap persamaan linear di kiri dengan nilai x yang benar di kanan!",
        "pairs": eq_pairs,
        "explanation": "Isolasikan x dengan memindahkan konstanta lalu membagi koefisien x untuk menemukan solusi yang cocok.",
        "topic_tag": "Cocokkan Aljabar"
    }));

    // Challenge 2: 4 Unique Percentage / Discount Calculations (unique prices!)
    let pct_options = [(10_i64, 50_000_i64), (20, 75_000), (25, 80_000), (50, 60_000), (30, 100_000), (40, 90_000)];
    let mut picked_pct = pct_options.to_vec();
    shuffle_vec(&mut picked_pct, rng);
    let mut used_res = HashSet::new();
    let mut disc_pairs = Vec::new();
    for (idx, (pct, price)) in picked_pct.into_iter().enumerate() {
        let cut = price * pct / 100;
        if used_res.contains(&cut) { continue; }
        used_res.insert(cut);
        disc_pairs.push(json!({
            "id": format!("pct_{}", idx),
            "left": format!("{}% dari {}", pct, format_rp(price)),
            "right": format_rp(cut),
        }));
        if disc_pairs.len() >= 4 { break; }
    }
    challenges.push(json!({
        "id": "sm_discount_dyn",
        "prompt": "Cocokkan perhitungan persentase dengan nominal Rupiah yang tepat!",
        "pairs": disc_pairs,
        "explanation": "Kalikan persentase (per 100) dengan harga pokok untuk mengetahui nominal potongan atau bagian.",
        "topic_tag": "Numerasi Belanja"
    }));

    // Challenge 3 & 4: Rich Cross-Subject Glossary Pairs (Ala Shopee Cocoki!)
    let glossary_bank = vec![
        ("Evaporasi", "Air menguap karena panas matahari", "IPA"),
        ("Kondensasi", "Uap air mendingin menjadi awan", "IPA"),
        ("Presipitasi", "Turunnya titik air sebagai hujan", "IPA"),
        ("Infiltrasi", "Air hujan meresap ke dalam tanah", "IPA"),
        ("Fotosintesis", "Tumbuhan mengolah cahaya jadi energi", "Biologi"),
        ("Klorofil", "Zat hijau daun penangkap cahaya", "Biologi"),
        ("Stomata", "Pori daun tempat keluar masuk gas", "Biologi"),
        ("Hipotesis", "Dugaan ilmiah sementara untuk diuji", "Sains"),
        ("Inflasi", "Harga barang naik terus secara umum", "Ekonomi"),
        ("Omzet", "Total uang masuk penjualan kotor", "Ekonomi"),
        ("Laba Bersih", "Pendapatan dikurangi seluruh biaya", "Ekonomi"),
        ("Phishing", "Penipuan tautan pencuri OTP & data", "Informatika"),
        ("Sinonim", "Kata berbeda bentuk bermakna mirip", "Bahasa"),
        ("Antonim", "Kata yang memiliki makna berlawanan", "Bahasa"),
        ("Deduktif", "Paragraf dengan gagasan utama di awal", "Bahasa"),
        ("Induktif", "Paragraf dengan kesimpulan di akhir", "Bahasa"),
        ("Musyawarah", "Berdiskusi bersama demi mufakat", "PPKn"),
        ("Gotong Royong", "Kerja bersama membantu kepentingan umum", "PPKn"),
        ("Bhinneka", "Beragam budaya dalam satu kesatuan", "PPKn"),
        ("Proklamasi", "Pengumuman resmi kemerdekaan bangsa", "Sejarah"),
    ];
    let mut shuffled_gloss = glossary_bank;
    shuffle_vec(&mut shuffled_gloss, rng);

    let round3: Vec<Value> = shuffled_gloss
        .iter()
        .take(4)
        .enumerate()
        .map(|(idx, (l, r, _))| json!({"id": format!("gl3_{}", idx), "left": l, "right": r}))
        .collect();
    challenges.push(json!({
        "id": "sm_glossary_a",
        "prompt": "Pasangkan setiap istilah pelajaran dengan definisi singkat yang tepat!",
        "pairs": round3,
        "explanation": "Menguasai definisi inti lintas mapel memudahkan memahami soal cerita dan bacaan.",
        "topic_tag": "Glosarium Cerdas"
    }));

    let round4: Vec<Value> = shuffled_gloss
        .iter()
        .skip(4)
        .take(4)
        .enumerate()
        .map(|(idx, (l, r, _))| json!({"id": format!("gl4_{}", idx), "left": l, "right": r}))
        .collect();
    if round4.len() >= 3 {
        challenges.push(json!({
            "id": "sm_glossary_b",
            "prompt": "Cocokkan konsep Sains, Ekonomi & PPKn berikut dengan artinya!",
            "pairs": round4,
            "explanation": "Setiap istilah kunci berpasangan dengan pengertian ilmiah atau sosial yang sesuai.",
            "topic_tag": "Wawasan Lintas Mapel"
        }));
    }

    challenges
}

async fn load_quiz_bank_challenges(
    pool: &PgPool,
    tenant_id: Uuid,
    game_mode: &str,
    rng: &mut u64,
) -> Vec<Value> {
    if game_mode != "CASE_DETECTIVE" && game_mode != "FACT_OR_MYTH" {
        return Vec::new();
    }

    let q_rows = sqlx::query(
        r#"
        SELECT qq.id, qq.question_text, qz.title AS quiz_title
        FROM quiz_questions qq
        JOIN quizzes qz ON qz.id = qq.quiz_id
        WHERE qz.tenant_id = $1
          AND qz.deleted_at IS NULL
          AND COALESCE(qz.is_active, true) = true
          AND LENGTH(TRIM(qq.question_text)) BETWEEN 15 AND 220
        ORDER BY qq.created_at DESC
        LIMIT 24
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if q_rows.is_empty() {
        return Vec::new();
    }

    let q_ids: Vec<Uuid> = q_rows.iter().map(|r| r.get("id")).collect();
    let c_rows = sqlx::query(
        "SELECT question_id, choice_text, is_correct FROM quiz_choices WHERE question_id = ANY($1) ORDER BY order_index ASC"
    )
    .bind(&q_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut choices_map: HashMap<Uuid, Vec<(String, bool)>> = HashMap::new();
    for cr in c_rows {
        let qid: Uuid = cr.get("question_id");
        let text: String = cr.try_get("choice_text").unwrap_or_default();
        let is_c: bool = cr.try_get("is_correct").unwrap_or(false);
        if !text.trim().is_empty() {
            choices_map.entry(qid).or_default().push((text.trim().to_string(), is_c));
        }
    }

    let mut indices: Vec<usize> = (0..q_rows.len()).collect();
    shuffle_vec(&mut indices, rng);

    let mut out = Vec::new();
    for idx in indices {
        if out.len() >= 2 { break; }
        let qr = &q_rows[idx];
        let qid: Uuid = qr.get("id");
        let q_text: String = qr.try_get("question_text").unwrap_or_default();
        let q_title: String = qr.try_get("quiz_title").unwrap_or_else(|_| "Kuis Kelas".to_string());
        let Some(choices) = choices_map.get(&qid) else { continue; };

        if game_mode == "CASE_DETECTIVE" && choices.len() >= 3 {
            let Some(corr_pos) = choices.iter().position(|(_, c)| *c) else { continue; };
            let opts: Vec<String> = choices.iter().take(4).map(|(t, _)| t.clone()).collect();
            if corr_pos >= opts.len() { continue; }
            let ch = json!({
                "id": format!("qb_{}", qid),
                "scenario": format!("Dari bank soal kuis '{}', pecahkan pertanyaan berikut:", q_title),
                "question": q_text,
                "options": opts,
                "correct_index": corr_pos,
                "explanation": format!("Jawaban ini diambil langsung dari materi evaluasi '{}'.", q_title),
                "topic_tag": "Bank Soal Kelas"
            });
            out.push(shuffle_case_options(ch, rng));
        } else if game_mode == "FACT_OR_MYTH" && choices.len() >= 2 {
            let pick_true = (next_rand(rng) % 2) == 0;
            let chosen = if pick_true {
                choices.iter().find(|(_, c)| *c)
            } else {
                choices.iter().find(|(_, c)| !*c)
            };
            let Some((choice_text, is_fact)) = chosen else { continue; };
            let right_ans = choices.iter().find(|(_, c)| *c).map(|(t, _)| t.as_str()).unwrap_or(choice_text);
            out.push(json!({
                "id": format!("qbf_{}", qid),
                "statement": format!("Dalam materi '{}', untuk '{}', jawaban yang tepat adalah: {}.", q_title, q_text, choice_text),
                "is_fact": *is_fact,
                "explanation": format!("Berdasarkan kuis '{}', jawaban yang benar adalah '{}'.", q_title, right_ans),
                "topic_tag": "Bank Soal Kelas"
            }));
        }
    }
    out
}

async fn try_generate_ai_quest(
    grade_band: &str,
    game_mode: &str,
    topic_hint: &str,
    subject_hint: &str,
    quest_date: NaiveDate,
    round: i32,
) -> Option<(String, String, String, Value)> {
    if game_mode == "MAP_PIN_HUNT" {
        return None;
    }

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
        "SPEED_MATCH" => r#"Format JSON WAJIB:
{
  "title": "judul cocokkan cepat (max 50 karakter)",
  "subtitle": "subjudul 1 kalimat",
  "subject_name": "nama mapel",
  "challenges": [
    {
      "id": "sm1",
      "prompt": "Pasangkan istilah di kiri dengan arti/hasil di kanan!",
      "pairs": [
        {"id": "p1", "left": "istilah 1", "right": "pasangan unik 1"},
        {"id": "p2", "left": "istilah 2", "right": "pasangan unik 2"},
        {"id": "p3", "left": "istilah 3", "right": "pasangan unik 3"},
        {"id": "p4", "left": "istilah 4", "right": "pasangan unik 4"}
      ],
      "explanation": "penjelasan ringkas",
      "topic_tag": "topik singkat"
    }
  ]
}
Buat TEPAT 3 item di array challenges, masing-masing memiliki TEPAT 4 pasangan left & right yang singkat dan 100% UNIK (tidak boleh ambigu)."#,
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

    let sys_prompt = "Kamu adalah perancang micro-learning game edukatif ala Shopee/GoPay untuk PKBM Indonesia (Kurikulum Merdeka). Buat soal UNIK, segar, kontekstual, dan tidak mengulang contoh klise. Jawab HANYA dengan JSON valid tanpa markdown fence ```json.";
    let user_prompt = format!(
        "Buat Misi Harian tanggal {} ronde ke-{} untuk jenjang {}.\nKonteks materi aktif sekolah: {} ({}).\nMode game: {}.\n{}",
        quest_date, round + 1, level_desc, topic_hint, subject_hint, game_mode, schema_instruction
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
        std::time::Duration::from_secs(16),
        call_nvidia_nim(messages, 0.85, 1700),
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
        "SPEED_MATCH" => challenges.iter().all(|c| {
            c.get("prompt").and_then(|v| v.as_str()).is_some()
                && c.get("pairs").and_then(|v| v.as_array()).map(|a| a.len() >= 3).unwrap_or(false)
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
    subject_hint: &str,
    rng: &mut u64,
    mut extra_bank_items: Vec<Value>,
) -> (String, String, String, Value) {
    match game_mode {
        "SPEED_MATCH" => {
            let items = gen_procedural_speed_match_items(rng, grade_band);
            (
                "Cocokkan Cepat: Arena Pasangan Dinamis".to_string(),
                "Ketuk pasangan kiri & kanan sebelum waktu habis untuk raih combo!".to_string(),
                if subject_hint.is_empty() { "Numerasi & Wawasan".to_string() } else { subject_hint.to_string() },
                Value::Array(items),
            )
        }
        "FACT_OR_MYTH" => {
            let d1 = (20 + (next_rand(rng) % 3) * 10) as i64;
            let d2 = (10 + (next_rand(rng) % 2) * 10) as i64;
            let naive_sum = d1 + d2;
            let actual_disc = d1 + (100 - d1) * d2 / 100;
            let num_a = (3 + (next_rand(rng) % 5)) as i64;
            let num_b = (4 + (next_rand(rng) % 6)) as i64;
            let prod_true = (next_rand(rng) % 2) == 0;
            let prod_shown = if prod_true { num_a * num_b * 10 } else { num_a * num_b * 10 + 10 };

            let mut all_pool = vec![
                json!({
                    "id": format!("pf_{}_{}", d1, d2),
                    "statement": format!("Diskon {}% lalu diskon tambahan {}% artinya total potongan harga persis {}%.", d1, d2, naive_sum),
                    "is_fact": false,
                    "explanation": format!("Mitos! Diskon kedua dihitung dari sisa harga setelah diskon pertama, jadi totalnya {}%.", actual_disc),
                    "topic_tag": "Numerasi Dinamis"
                }),
                json!({
                    "id": format!("pf_mul_{}_{}", num_a, num_b),
                    "statement": format!("Hasil dari {} x {} = {}.", num_a * 10, num_b, prod_shown),
                    "is_fact": prod_true,
                    "explanation": format!("Hasil perkalian {} x {} yang benar adalah {}.", num_a * 10, num_b, num_a * num_b * 10),
                    "topic_tag": "Hitung Cepat"
                }),
                json!({"id":"f1","statement":"Inflasi berarti nilai uang semakin menguat sehingga harga barang cenderung turun terus.","is_fact":false,"explanation":"Mitos! Inflasi justru terjadi saat harga barang naik secara umum sehingga daya beli uang menurun.","topic_tag":"Ekonomi"}),
                json!({"id":"f3","statement":"Kalimat utama dalam paragraf deduktif terletak di bagian awal paragraf.","is_fact":true,"explanation":"Fakta! Paragraf deduktif dimulai dari gagasan utama lalu diikuti kalimat penjelas.","topic_tag":"Bahasa Indonesia"}),
                json!({"id":"f4","statement":"Energi matahari termasuk sumber daya alam yang dapat diperbarui karena tersedia terus secara alami.","is_fact":true,"explanation":"Fakta! Energi surya, angin, dan air adalah energi terbarukan ramah lingkungan.","topic_tag":"IPA"}),
                json!({"id":"f5","statement":"Gotong royong dan musyawarah mufakat merupakan cerminan nilai sila ke-4 Pancasila dalam masyarakat.","is_fact":true,"explanation":"Fakta! Musyawarah untuk mencapai mufakat berakar langsung pada sila ke-4 Pancasila.","topic_tag":"PPKn"}),
                json!({"id":"f6","statement":"Semua bakteri di tubuh manusia selalu berbahaya dan harus dimusnahkan seluruhnya.","is_fact":false,"explanation":"Mitos! Ada bakteri baik di usus yang justru membantu mencerna makanan dan menjaga imunitas.","topic_tag":"Biologi"}),
                json!({"id":"f7","statement":"Dalam usaha kecil, omzet penjualan sama artinya dengan laba bersih yang boleh langsung dihabiskan.","is_fact":false,"explanation":"Mitos! Omzet adalah total uang masuk kotor; laba bersih didapat setelah dikurangi seluruh modal dan biaya.","topic_tag":"Kewirausahaan"}),
                json!({"id":"g1","statement":"Air mendidih di puncak gunung yang sangat tinggi pada suhu lebih rendah dari 100°C.","is_fact":true,"explanation":"Fakta! Tekanan udara yang lebih rendah di dataran tinggi membuat titik didih air menurun.","topic_tag":"Fisika"}),
                json!({"id":"g2","statement":"Kata 'di' pada 'di rumah' ditulis terpisah karena menunjukkan kata depan tempat.","is_fact":true,"explanation":"Fakta! 'Di' sebagai preposisi penunjuk tempat ditulis terpisah, berbeda dengan imbuhan pasif seperti 'dimakan'.","topic_tag":"Bahasa Indonesia"}),
                json!({"id":"g3","statement":"Nilai tukar Rupiah melemah selalu menguntungkan warga yang membeli bahan baku impor.","is_fact":false,"explanation":"Mitos! Saat Rupiah melemah, harga barang impor justru menjadi lebih mahal dalam Rupiah.","topic_tag":"Ekonomi"}),
                json!({"id":"g4","statement":"Fotosintesis pada tumbuhan hijau melepaskan gas oksigen ke udara.","is_fact":true,"explanation":"Fakta! Tumbuhan menyerap CO2 + air dan melepaskan oksigen dengan bantuan cahaya matahari.","topic_tag":"IPA"}),
                json!({"id":"g5","statement":"Pajak yang dibayar warga negara hanya digunakan untuk gaji pegawai dan tidak pernah untuk jalan atau sekolah.","is_fact":false,"explanation":"Mitos! APBN dari pajak membiayai infrastruktur publik, subsidi pendidikan, kesehatan, dan fasilitas umum.","topic_tag":"IPS"}),
                json!({"id":"g6","statement":"Password akun belajar sebaiknya sama persis dengan tanggal lahir agar mudah diingat.","is_fact":false,"explanation":"Mitos! Tanggal lahir mudah ditebak; gunakan kombinasi huruf, angka, dan karakter unik.","topic_tag":"Informatika"}),
            ];
            shuffle_vec(&mut all_pool, rng);
            let mut items = Vec::new();
            items.append(&mut extra_bank_items);
            for item in all_pool {
                if items.len() >= 5 { break; }
                items.push(item);
            }
            shuffle_vec(&mut items, rng);
            let title = if grade_band == "PAKET_A" {
                "Fakta atau Mitos Pintar: Tantangan Cepat".to_string()
            } else {
                "Fakta atau Mitos: Sains, Numerasi & Wawasan".to_string()
            };
            (
                title,
                "Geser kanan jika Fakta, geser kiri jika Mitos — soal baru tiap ronde!".to_string(),
                if subject_hint.is_empty() { "Lintas Mapel".to_string() } else { subject_hint.to_string() },
                Value::Array(items),
            )
        }
        "CASE_DETECTIVE" => {
            let proc_cases = gen_procedural_case_items(rng, 2);
            let mut curated_pool = vec![
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
            ]
            .into_iter()
            .map(|c| shuffle_case_options(c, rng))
            .collect::<Vec<Value>>();
            shuffle_vec(&mut curated_pool, rng);

            let mut items = Vec::new();
            items.append(&mut extra_bank_items);
            for pc in proc_cases {
                if items.len() < 4 { items.push(pc); }
            }
            for cc in curated_pool {
                if items.len() < 4 { items.push(cc); }
            }
            shuffle_vec(&mut items, rng);
            (
                "Detektif Kasus Nyata: Tantangan Cerdas".to_string(),
                "Baca kasus kehidupan sehari-hari dan pilih keputusan terbaik!".to_string(),
                if subject_hint.is_empty() { "Keterampilan Terapan".to_string() } else { subject_hint.to_string() },
                Value::Array(items),
            )
        }
        "SEQUENCE_ORDER" => {
            let proc_seq = gen_procedural_sequence_items(rng, 2);
            let mut logic_pool = vec![
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
            shuffle_vec(&mut logic_pool, rng);
            let mut items = proc_seq;
            if let Some(first_logic) = logic_pool.into_iter().next() {
                items.push(first_logic);
            }
            shuffle_vec(&mut items, rng);
            (
                "Rangkai Logika: Susun Urutan yang Tepat".to_string(),
                "Atur setiap blok langkah dari posisi 1 sampai 4 dengan soal dinamis!".to_string(),
                "Matematika & Logika".to_string(),
                Value::Array(items),
            )
        }
        "MAP_PIN_HUNT" => {
            let routine_idx = (next_rand(rng) % 4) as usize;
            let routine_prompts = [
                ("r1", "HUNT: Klik jam weker digital 05:00 tempat Andi bangun pagi!", "Pukul 05:00 pagi ditunjukkan oleh jam digital hijau di atas meja samping kasur."),
                ("r2", "HUNT: Klik tokoh Andi yang sedang bangun pagi dan meregangkan badan di tempat tidur!", "Andi bangun pukul 05:00 dengan semangat sebelum bersiap ke sekolah."),
                ("r3", "HUNT: Klik seragam sekolah SMP putih-biru yang sudah disiapkan Andi di gantungan dinding!", "Menyiapkan seragam putih-biru sejak malam membantu berangkat sekolah lebih disiplin."),
                ("r4", "HUNT: Klik perlengkapan olahraga (bola sepak) di pojok kanan kamar Andi!", "Bola sepak di sudut kamar disiapkan untuk kegiatan olahraga ekstrakurikuler."),
            ];
            let (rt_id, rt_prompt, rt_expl) = routine_prompts[routine_idx];

            let prok_idx = (next_rand(rng) % 4) as usize;
            let prok_prompts = [
                ("h3", "TEBAK LOKASI: Klik titik pembacaan Teks Proklamasi oleh Soekarno-Hatta!", "Ir. Soekarno didampingi Drs. Moh. Hatta membacakan Teks Proklamasi di depan teras rumah Pegangsaan Timur 56."),
                ("h1", "TEBAK LOKASI: Klik tiang pengibaran Sang Saka Merah Putih pada upacara Proklamasi!", "Bendera Merah Putih jahitan Ibu Fatmawati dikibarkan di tiang bambu halaman depan setelah pembacaan teks."),
                ("h2", "TEBAK LOKASI: Klik area mikrofon yang digunakan untuk menyiarkan suara kemerdekaan!", "Mikrofon corong halaman depan membantu suara pembacaan naskah terdengar jelas oleh seluruh hadirin."),
                ("h4", "TEBAK LOKASI: Klik barisan pemuda dan tokoh tamu undangan yang menyaksikan Proklamasi!", "Para pemuda, pejuang, dan warga menyaksikan langsung momen bersejarah 17 Agustus 1945."),
            ];
            let (pk_id, pk_prompt, pk_expl) = prok_prompts[prok_idx];

            let mut items = vec![
                json!({
                    "id": "mp_cirebon",
                    "scene_id": "cirebon_keraton_map",
                    "interaction_type": "MAP_TAGS",
                    "prompt": "Sematkan tag ke lokasi yang tepat pada peta untuk menandai wilayah Kesultanan Cirebon dan keraton-keratonnya!",
                    "pins": [
                        {"id": "p1", "label": "Kesultanan Cirebon (Pusat)", "step_label": "Tag Cirebon Pusat", "x": 0.52, "y": 0.49},
                        {"id": "p2", "label": "Kasepuhan (Keraton)", "step_label": "Tag Kasepuhan", "x": 0.47, "y": 0.40},
                        {"id": "p3", "label": "Kanoman (Keraton)", "step_label": "Tag Kanoman", "x": 0.43, "y": 0.55},
                        {"id": "p4", "label": "Kacirebonan (Keraton)", "step_label": "Tag Kacirebonan", "x": 0.53, "y": 0.58}
                    ],
                    "explanation": "Keraton Kasepuhan, Kanoman, dan Kacirebonan merupakan pusat sejarah Kesultanan Cirebon di pesisir utara Jawa Barat.",
                    "topic_tag": "Geografi & Sejarah"
                }),
                json!({
                    "id": format!("mp_prok_{}", pk_id),
                    "scene_id": "proklamasi_1945",
                    "interaction_type": "SCENE_HOTSPOT",
                    "prompt": pk_prompt,
                    "pins": [
                        {"id": "h1", "label": "Tiang Bendera Merah Putih", "step_label": "Tiang Bendera", "x": 0.26, "y": 0.46, "is_target": pk_id == "h1"},
                        {"id": "h2", "label": "Mikrofon Halaman Depan", "step_label": "Area Mikrofon", "x": 0.38, "y": 0.60, "is_target": pk_id == "h2"},
                        {"id": "h3", "label": "Podium Pembacaan Teks Proklamasi", "step_label": "Pembaca Teks", "x": 0.50, "y": 0.55, "is_target": pk_id == "h3"},
                        {"id": "h4", "label": "Barisan Pemuda & Tamu Undangan", "step_label": "Barisan Tamu", "x": 0.78, "y": 0.58, "is_target": pk_id == "h4"}
                    ],
                    "explanation": pk_expl,
                    "topic_tag": "Sejarah Nasional"
                }),
                json!({
                    "id": format!("mp_routine_{}", rt_id),
                    "scene_id": "morning_routine_room",
                    "interaction_type": "SCENE_HOTSPOT",
                    "prompt": rt_prompt,
                    "pins": [
                        {"id": "r1", "label": "Jam Digital 05:00", "step_label": "Jam 05:00", "x": 0.135, "y": 0.62, "is_target": rt_id == "r1"},
                        {"id": "r2", "label": "Andi Bangun Tidur", "step_label": "Kasur Andi", "x": 0.45, "y": 0.46, "is_target": rt_id == "r2"},
                        {"id": "r3", "label": "Seragam Sekolah", "step_label": "Seragam", "x": 0.89, "y": 0.42, "is_target": rt_id == "r3"},
                        {"id": "r4", "label": "Bola Sepak", "step_label": "Bola Sepak", "x": 0.92, "y": 0.86, "is_target": rt_id == "r4"}
                    ],
                    "explanation": rt_expl,
                    "topic_tag": "Bahasa Inggris & Rutinitas"
                }),
                json!({
                    "id": "mp_fotosintesis",
                    "scene_id": "photosynthesis_scene",
                    "interaction_type": "MAP_TAGS",
                    "prompt": "Sematkan tag komponen Fotosintesis ke titik yang tepat pada diagram daun dan tanaman!",
                    "pins": [
                        {"id": "f1", "label": "Energi Cahaya Matahari", "step_label": "Tag Matahari", "x": 0.24, "y": 0.25},
                        {"id": "f2", "label": "Kloroplas pada Daun Hijau", "step_label": "Tag Kloroplas", "x": 0.54, "y": 0.45},
                        {"id": "f3", "label": "Penyerapan Air & Mineral Akar", "step_label": "Tag Akar Air", "x": 0.48, "y": 0.82},
                        {"id": "f4", "label": "Pelepasan Gas Oksigen (O₂)", "step_label": "Tag Oksigen", "x": 0.78, "y": 0.34}
                    ],
                    "explanation": "Tumbuhan menyerap air lewat akar dan CO₂ lewat stomata, lalu mengolahnya di kloroplas dengan cahaya matahari menghasilkan energi & O₂.",
                    "topic_tag": "IPA Biologi"
                }),
            ];
            shuffle_vec(&mut items, rng);
            (
                "Eksplorasi Peta & Lokasi Interaktif".to_string(),
                "Sematkan tag lokasi pada peta dan temukan target dinamis di gambar!".to_string(),
                "Geografi, Sejarah & Sains".to_string(),
                Value::Array(items),
            )
        }
        _ => {
            let mut pool = vec![
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
                json!({
                    "id": "w6",
                    "answer": "KLOROFIL",
                    "clues": [
                        "Ditemukan di dalam organel sel tumbuhan yang disebut kloroplas.",
                        "Memberi warna hijau khas pada daun sehat.",
                        "Berfungsi menyerap energi cahaya matahari saat fotosintesis."
                    ],
                    "acceptable_aliases": ["KLOROFIL"],
                    "explanation": "Klorofil adalah pigmen hijau pada tumbuhan yang menangkap energi cahaya matahari.",
                    "topic_tag": "IPA Biologi"
                }),
                json!({
                    "id": "w7",
                    "answer": "ANTONIM",
                    "clues": [
                        "Pasangan kata dalam pelajaran kosakata Bahasa Indonesia.",
                        "Menunjukkan hubungan arti yang saling bertolak belakang.",
                        "Contohnya: panas >< dingin, naik >< turun, hemat >< boros."
                    ],
                    "acceptable_aliases": ["ANTONIM"],
                    "explanation": "Antonim adalah dua kata yang maknanya berlawanan satu sama lain.",
                    "topic_tag": "Bahasa Indonesia"
                }),
                json!({
                    "id": "w8",
                    "answer": "EVAPORASI",
                    "clues": [
                        "Tahap pembuka dalam siklus air (hidrologi) di bumi.",
                        "Terjadi ketika permukaan laut, sungai, dan danau terkena panas matahari.",
                        "Mengubah air cair menjadi uap air yang naik ke atmosfer."
                    ],
                    "acceptable_aliases": ["EVAPORASI", "PENGUAPAN"],
                    "explanation": "Evaporasi adalah proses penguapan air permukaan menuju atmosfer akibat panas matahari.",
                    "topic_tag": "IPA & Geografi"
                }),
                json!({
                    "id": "w9",
                    "answer": "PHISHING",
                    "clues": [
                        "Istilah keamanan siber dalam Literasi Digital modern.",
                        "Sering berkedok tautan hadiah, surat bank palsu, atau formulir undian.",
                        "Bertujuan mencuri password, PIN, atau kode OTP korban."
                    ],
                    "acceptable_aliases": ["PHISHING", "PISHING"],
                    "explanation": "Phishing adalah upaya penipuan digital dengan menyamar sebagai pihak resmi untuk mencuri data sensitif.",
                    "topic_tag": "Informatika"
                }),
                json!({
                    "id": "w10",
                    "answer": "STOMATA",
                    "clues": [
                        "Struktur mikroskopis yang kebanyakan berada di permukaan bawah daun.",
                        "Bisa membuka dan menutup seperti mulut kecil.",
                        "Tempat masuknya CO2 dan keluarnya oksigen serta uap air pada tumbuhan."
                    ],
                    "acceptable_aliases": ["STOMATA"],
                    "explanation": "Stomata adalah pori-pori daun yang mengatur pertukaran gas dan transpirasi tumbuhan.",
                    "topic_tag": "Biologi"
                }),
            ];
            shuffle_vec(&mut pool, rng);
            pool.truncate(4);
            (
                "Tebak Istilah Harian: Uji Wawasan".to_string(),
                "Pecahkan kata kunci materi hanya dengan sedikit petunjuk untuk XP maksimal!".to_string(),
                "Wawasan Lintas Mapel".to_string(),
                Value::Array(pool),
            )
        }
    }
}

async fn award_extra_xp(pool: &PgPool, tenant_id: Uuid, student_id: Uuid, action_type: &str, xp_amount: i32, desc: &str) -> (i64, i32, i32, bool) {
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
        "SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1"
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
            r.try_get::<Option<NaiveDate>, _>("last_activity_date").ok().flatten(),
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
    let mut rng = seed_for_quest(today, round, actor.id, &requested_mode, "");

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
    rng = seed_for_quest(today, round, actor.id, &requested_mode, &grade_band);

    // 2. Multi-material curriculum rotation (fetch up to 12 materials and rotate by seed!)
    let topic_rows = sqlx::query(
        r#"
        SELECT m.title, COALESCE(sub.name, 'Umum') AS subject_name
        FROM learning_materials m
        LEFT JOIN subjects sub ON sub.id = m.subject_id AND sub.tenant_id = m.tenant_id
        WHERE m.tenant_id = $1
          AND m.deleted_at IS NULL
          AND ($2::uuid IS NULL OR m.class_id IS NULL OR m.class_id = $2)
        ORDER BY m.created_at DESC
        LIMIT 12
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(class_id_opt)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let (topic_hint, subject_hint) = if topic_rows.is_empty() {
        ("Pembelajaran Kontekstual PKBM".to_string(), "Literasi & Numerasi".to_string())
    } else {
        let pick_idx = (next_rand(&mut rng) as usize) % topic_rows.len();
        let r = &topic_rows[pick_idx];
        (
            r.try_get::<String, _>("title").unwrap_or_else(|_| "Pembelajaran Kontekstual".to_string()),
            r.try_get::<String, _>("subject_name").unwrap_or_else(|_| "Literasi & Numerasi".to_string()),
        )
    };

    // 3. Round == 0 uses daily cached quest per (tenant, date, grade_band, mode); Round >= 1 generates fresh ephemeral round!
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
        .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?
    } else {
        None
    };

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
            let quiz_bank_items = load_quiz_bank_challenges(&ctx.pool, req_ctx.tenant_id, &requested_mode, &mut rng).await;
            let (gen_title, gen_subtitle, gen_subject, gen_challenges, gen_source) =
                if requested_mode == "MAP_PIN_HUNT" {
                    let (t, sub, subj, ch) = build_curated_quest(&grade_band, &requested_mode, &subject_hint, &mut rng, quiz_bank_items);
                    (t, sub, subj, ch, "VISUAL_MAP_BANK".to_string())
                } else {
                    match try_generate_ai_quest(&grade_band, &requested_mode, &topic_hint, &subject_hint, today, round).await {
                        Some((t, sub, subj, mut ch)) => {
                            if requested_mode == "CASE_DETECTIVE" {
                                if let Some(arr) = ch.as_array_mut() {
                                    for item in arr.iter_mut() {
                                        let shuffled = shuffle_case_options(item.clone(), &mut rng);
                                        *item = shuffled;
                                    }
                                }
                            }
                            (t, sub, subj, ch, "AI_CURRICULUM".to_string())
                        }
                        None => {
                            let src = if !quiz_bank_items.is_empty() {
                                "QUIZ_AND_DYNAMIC_ENGINE"
                            } else {
                                "DYNAMIC_ENGINE"
                            };
                            let (t, sub, subj, ch) = build_curated_quest(&grade_band, &requested_mode, &subject_hint, &mut rng, quiz_bank_items);
                            (t, sub, subj, ch, src.to_string())
                        }
                    }
                };

            let base_xp = if round > 0 {
                20
            } else if requested_mode == featured_mode {
                80
            } else {
                40
            };

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
            } else {
                (
                    Uuid::new_v4(),
                    format!("{} (Ronde #{})", gen_title, round + 1),
                    gen_subtitle,
                    gen_subject,
                    gen_source,
                    base_xp,
                    gen_challenges,
                )
            }
        }
    };

    // 4. Student attempt + gamification stats + Shiba Pet + Chest claim state
    let mut my_attempt: Option<DailyQuestAttemptSummary> = None;
    let mut completed_modes_today: Vec<String> = Vec::new();
    let mut total_xp: i64 = 0;
    let mut level: i32 = 1;
    let mut streak_days: i32 = 0;
    let mut pet_status: Option<ShibaPetStatus> = None;
    let mut chest_claimed_today = false;

    if let Some(sid) = student_id_opt {
        if round == 0 {
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

        pet_status = Some(load_or_create_pet(&ctx.pool, req_ctx.tenant_id, sid).await);

        chest_claimed_today = sqlx::query(
            "SELECT 1 FROM daily_quest_chest_claims WHERE student_id = $1 AND claim_date = $2 LIMIT 1"
        )
        .bind(sid)
        .bind(today)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .is_some();
    } else {
        pet_status = Some(build_pet_status(1, 25, 0, 2, 1));
    }

    // 5. Daily Leaderboard for official daily quest
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

    let all_modes = ["MAP_PIN_HUNT", "SPEED_MATCH", "FACT_OR_MYTH", "SEQUENCE_ORDER", "CASE_DETECTIVE", "CONCEPT_GUESS"];
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
        "SELECT id, title, xp_reward, game_mode FROM daily_quests WHERE id = $1 AND tenant_id = $2 LIMIT 1"
    )
    .bind(payload.quest_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let is_bonus_ephemeral = quest_row.is_none() || payload.round_number > 0;
    let (quest_title, base_reward) = match quest_row {
        Some(r) => (r.get::<String, _>("title"), r.get::<i32, _>("xp_reward")),
        None => ("Misi Ronde Dinamis".to_string(), 20),
    };

    let safe_total = payload.total_count.max(1);
    let safe_correct = payload.correct_count.clamp(0, safe_total);
    let score_percent = ((safe_correct as f64 / safe_total as f64) * 100.0).round() as i32;
    let combo_bonus = (payload.max_combo.max(0) * 5).min(25);
    let completion_bonus = (base_reward / 3).max(15);
    let accuracy_xp = ((base_reward as f64 * (safe_correct as f64 / safe_total as f64)).round() as i32).max(5);
    let computed_xp = if is_bonus_ephemeral {
        15
    } else {
        (completion_bonus + accuracy_xp + combo_bonus).clamp(20, 140)
    };

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
                biscuits_earned: if safe_correct > 0 { 2 } else { 0 },
                pet_status: Some(build_pet_status(1, 25, 0, 2, 1)),
                message: format!(
                    "Mode Pratinjau selesai dengan akurasi {}% (simulasi +{} XP).",
                    score_percent, computed_xp
                ),
            },
            req_ctx.request_id,
        )));
    };

    let student_id: Uuid = st_row.get("id");
    let _ = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;

    // Bonus / ephemeral round reward (Biskuit Shiba + up to 3 bonus XP rounds/day)
    if is_bonus_ephemeral {
        let bonus_row = sqlx::query(
            "SELECT COUNT(*)::int AS cnt, COALESCE(SUM(biscuits_earned), 0)::int AS bis FROM daily_quest_bonus_logs WHERE student_id = $1 AND bonus_date = $2"
        )
        .bind(student_id)
        .bind(today)
        .fetch_one(&ctx.pool)
        .await;

        let (bonus_cnt, bonus_bis_today): (i32, i32) = match bonus_row {
            Ok(r) => (r.try_get("cnt").unwrap_or(0), r.try_get("bis").unwrap_or(0)),
            Err(_) => (0, 0),
        };

        let biscuits_earned = if safe_correct > 0 && bonus_bis_today < 6 { 1 } else { 0 };
        let xp_bonus = if safe_correct > 0 && bonus_cnt < 3 { 15 } else { 0 };

        if biscuits_earned > 0 || xp_bonus > 0 {
            let _ = sqlx::query(
                "INSERT INTO daily_quest_bonus_logs (id, tenant_id, student_id, bonus_date, biscuits_earned, created_at) VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())"
            )
            .bind(req_ctx.tenant_id)
            .bind(student_id)
            .bind(today)
            .bind(biscuits_earned)
            .execute(&ctx.pool)
            .await;

            if biscuits_earned > 0 {
                let _ = sqlx::query(
                    "UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1"
                )
                .bind(student_id)
                .bind(biscuits_earned)
                .execute(&ctx.pool)
                .await;
            }
        }

        let (total_xp, level, streak_days, leveled_up) = if xp_bonus > 0 {
            award_extra_xp(&ctx.pool, req_ctx.tenant_id, student_id, "DAILY_QUEST_BONUS", xp_bonus, &format!("Ronde Dinamis ({}%)", score_percent)).await
        } else {
            let cur = sqlx::query("SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1 AND tenant_id = $2")
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
            format!("Ronde Baru selesai ({}%)! Raih +{} XP & +{} Biskuit Shiba!", score_percent, xp_bonus, biscuits_earned)
        } else {
            format!("Ronde Latihan selesai ({}%)! Batas bonus harian tercapai, terus asah kemampuanmu!", score_percent)
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

    // Check if student already completed official daily quest today
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
                    "Latihan ulang tuntas ({}%)! Tekan 'Main Ronde Baru' untuk dapat soal fresh & Biskuit Shiba.",
                    score_percent
                ),
            },
            req_ctx.request_id,
        )));
    }

    // Record first completion & award XP + 2 Biskuit Shiba atomically
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
        "UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1"
    )
    .bind(student_id)
    .bind(biscuits_earned)
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
        format!("Luar biasa! Naik Level {} & raih +{} XP + 2 Biskuit Shiba!", new_level, computed_xp)
    } else {
        format!("Misi Harian tuntas ({}%)! +{} XP & +2 Biskuit Shiba!", score_percent, computed_xp)
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
            FeedPetResponse {
                fed: false,
                evolved: false,
                bonus_xp_earned: 0,
                total_xp: 0,
                level: 1,
                pet_status: build_pet_status(1, 50, 1, 1, 1),
                message: "Mode Pratinjau: Shiba senang menerima biskuitmu!".to_string(),
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
        let (txp, lvl): (i64, i32) = cur.map(|r| (r.get("total_xp"), r.get("level"))).unwrap_or((0, 1));
        return Ok(Json(ApiResponse::success(
            FeedPetResponse {
                fed: false,
                evolved: false,
                bonus_xp_earned: 0,
                total_xp: txp,
                level: lvl,
                pet_status: pet,
                message: "Biskuit habis! Selesaikan Misi Harian atau Ronde Baru untuk dapat Biskuit Shiba.".to_string(),
            },
            req_ctx.request_id,
        )));
    }

    let next_points = pet.food_points + 25;
    let evolved = next_points >= 100;
    let final_stage = if evolved { (pet.pet_stage + 1).min(4) } else { pet.pet_stage };
    let final_points = if evolved { next_points - 100 } else { next_points };
    let next_tokens = if evolved { pet.fifty_fifty_tokens + 1 } else { pet.fifty_fifty_tokens };

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
        if evolved { "Evolusi Shiba Pet!" } else { "Merawat Shiba Pet" },
    )
    .await;

    let updated_pet = load_or_create_pet(&ctx.pool, req_ctx.tenant_id, student_id).await;
    let msg = if evolved {
        format!("YEAY! Shiba berevolusi jadi {}! Bonus +{} XP & +1 Token 50:50!", updated_pet.stage_title, bonus_xp)
    } else {
        format!("Nyam! Shiba makan 1 biskuit (+{} XP). Energi Shiba: {}/100.", bonus_xp, updated_pet.food_points)
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
        "SELECT reward_type, reward_amount, reward_label FROM daily_quest_chest_claims WHERE student_id = $1 AND claim_date = $2 LIMIT 1"
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
        let (txp, lvl): (i64, i32) = cur.map(|r| (r.get("total_xp"), r.get("level"))).unwrap_or((0, 1));
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
        1 => ("BISCUIT", 2, "+2 Biskuit Shiba Spesial".to_string()),
        2 => ("POWERUP", 1, "+1 Token 50:50 & +1 Biskuit".to_string()),
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
        let (t, l, _, _) = award_extra_xp(&ctx.pool, req_ctx.tenant_id, student_id, "DAILY_CHEST", reward_amount, &reward_label).await;
        (t, l)
    } else {
        if reward_type == "BISCUIT" {
            let _ = sqlx::query("UPDATE student_shiba_pets SET biscuits = biscuits + $2, updated_at = NOW() WHERE student_id = $1")
                .bind(student_id)
                .bind(reward_amount)
                .execute(&ctx.pool)
                .await;
        } else {
            let _ = sqlx::query("UPDATE student_shiba_pets SET fifty_fifty_tokens = fifty_fifty_tokens + 1, biscuits = biscuits + 1, updated_at = NOW() WHERE student_id = $1")
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
        cur.map(|r| (r.get("total_xp"), r.get("level"))).unwrap_or((0, 1))
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
