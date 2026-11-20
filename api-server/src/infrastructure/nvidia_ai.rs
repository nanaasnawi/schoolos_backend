use school_core::common::error::{ApplicationError, DomainError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_NVIDIA_MODEL: &str = "nvidia/nemotron-3.5-lightning-30b-a3b";
const DEFAULT_NVIDIA_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnedChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

fn get_config() -> Result<(String, String, String), ApplicationError> {
    // Strictly load NVIDIA API key from environment variable (Never hardcode secrets!)
    let key = std::env::var("NVIDIA_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::System(
                "NVIDIA_API_KEY environment variable is not configured or revoked".to_string(),
            ))
        })?;

    let model = std::env::var("NVIDIA_MODEL").unwrap_or_else(|_| DEFAULT_NVIDIA_MODEL.to_string());
    let url = std::env::var("NVIDIA_API_URL").unwrap_or_else(|_| DEFAULT_NVIDIA_URL.to_string());
    Ok((key, model, url))
}

static SHARED_AI_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn get_ai_client() -> &'static reqwest::Client {
    SHARED_AI_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// Call NVIDIA NIM API with model fallback
pub async fn call_nvidia_nim(
    messages: Vec<ChatMessage<'_>>,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, ApplicationError> {
    let (api_key, configured_model, api_url) = get_config()?;
    let client = get_ai_client();

    let mut candidate_models = vec![configured_model.as_str()];
    let active_models = [
        "nvidia/nemotron-3.5-lightning-30b-a3b",
        "nvidia/nemotron-3.5-super-vl-120b-a12b",
        "nvidia/nemotron-3-ultra-550b-a55b",
    ];
    for m in active_models {
        if !candidate_models.contains(&m) {
            candidate_models.push(m);
        }
    }

    let mut last_error = String::new();

    for candidate in candidate_models {
        let payload = ChatCompletionRequest {
            model: candidate,
            messages: messages.clone(),
            temperature,
            max_tokens,
            reasoning_effort: Some("none"),
            stream: None,
        };

        let resp = match client
            .post(&api_url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Failed to send request to NVIDIA NIM ({candidate}): {e}");
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            last_error = format!("NVIDIA NIM API ({candidate}) responded with HTTP {status}: {err_body}");
            continue;
        }

        let parsed: ChatCompletionResponse = match resp.json().await {
            Ok(p) => p,
            Err(e) => {
                last_error = format!("Failed to parse NVIDIA NIM response ({candidate}): {e}");
                continue;
            }
        };

        if let Some(choice) = parsed.choices.into_iter().next() {
            let msg = choice.message;
            if let Some(content) = msg.content.filter(|c| !c.trim().is_empty()) {
                return Ok(content);
            }
            if let Some(reasoning) = msg.reasoning_content.filter(|r| !r.trim().is_empty()) {
                if reasoning.contains('{') && reasoning.contains('}') {
                    return Ok(reasoning);
                }
            }
        }
    }

    Err(ApplicationError::Internal(if last_error.is_empty() {
        "NVIDIA NIM returned empty response across all candidate models".to_string()
    } else {
        last_error
    }))
}

fn extract_stream_delta_from_sse_line(line: &str) -> Option<Result<String, ()>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return None;
    }
    let data = trimmed.strip_prefix("data:")?.trim();
    if data == "[DONE]" {
        return Some(Err(()));
    }
    let val: serde_json::Value = serde_json::from_str(data).ok()?;
    let choices = val.get("choices")?.as_array()?;
    let first = choices.first()?;
    let delta_text = first
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .or_else(|| {
            first
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
        })?;
    if delta_text.is_empty() {
        None
    } else {
        Some(Ok(delta_text.to_string()))
    }
}

/// Streams NVIDIA NIM chat completion token deltas in real time over `tx`.
pub async fn stream_nvidia_nim(
    messages: Vec<OwnedChatMessage>,
    temperature: f32,
    max_tokens: u32,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<String, ApplicationError> {
    let (api_key, configured_model, api_url) = get_config()?;
    let client = get_ai_client();

    let mut candidate_models = vec![configured_model.as_str()];
    let active_models = [
        "nvidia/nemotron-3.5-lightning-30b-a3b",
        "nvidia/nemotron-3.5-super-vl-120b-a12b",
        "nvidia/nemotron-3-ultra-550b-a55b",
    ];
    for m in active_models {
        if !candidate_models.contains(&m) {
            candidate_models.push(m);
        }
    }

    let msg_refs: Vec<ChatMessage<'_>> = messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.as_str(),
            content: m.content.as_str(),
        })
        .collect();

    let mut last_error = String::new();

    for candidate in candidate_models {
        let payload = ChatCompletionRequest {
            model: candidate,
            messages: msg_refs.clone(),
            temperature,
            max_tokens,
            reasoning_effort: Some("none"),
            stream: Some(true),
        };

        let mut resp = match client
            .post(&api_url)
            .timeout(Duration::from_secs(90))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Failed to stream from NVIDIA NIM ({candidate}): {e}");
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            last_error = format!("NVIDIA NIM stream ({candidate}) HTTP {status}: {err_body}");
            continue;
        }

        let mut buf: Vec<u8> = Vec::with_capacity(4096);
        let mut streamed_any = false;
        let mut stream_done = false;

        while let Some(chunk_res) = match resp.chunk().await {
            Ok(opt) => opt.map(Ok),
            Err(e) => Some(Err(e)),
        } {
            let chunk = match chunk_res {
                Ok(c) => c,
                Err(e) => {
                    last_error = format!("Stream read interrupted ({candidate}): {e}");
                    break;
                }
            };

            buf.extend_from_slice(&chunk);

            while let Some(nl_pos) = buf.iter().position(|&b| b == b'\n') {
                let raw_line: Vec<u8> = buf.drain(..=nl_pos).collect();
                let line = String::from_utf8_lossy(&raw_line);
                if let Some(parsed) = extract_stream_delta_from_sse_line(&line) {
                    match parsed {
                        Ok(delta) => {
                            streamed_any = true;
                            if tx.send(delta).await.is_err() {
                                // Client disconnected
                                return Ok(candidate.to_string());
                            }
                        }
                        Err(()) => {
                            stream_done = true;
                            break;
                        }
                    }
                }
            }

            if stream_done {
                break;
            }
        }

        // Flush any trailing unterminated SSE line
        if !stream_done && !buf.is_empty() {
            let line = String::from_utf8_lossy(&buf);
            if let Some(Ok(delta)) = extract_stream_delta_from_sse_line(&line) {
                streamed_any = true;
                let _ = tx.send(delta).await;
            }
        }

        if streamed_any {
            return Ok(candidate.to_string());
        }
    }

    Err(ApplicationError::Internal(if last_error.is_empty() {
        "NVIDIA NIM stream returned empty response across all candidate models".to_string()
    } else {
        last_error
    }))
}

pub fn extract_clean_json(text: &str) -> &str {
    let mut trimmed = text.trim();
    if trimmed.starts_with("```json") {
        trimmed = trimmed.trim_start_matches("```json").trim();
    } else if trimmed.starts_with("```") {
        trimmed = trimmed.trim_start_matches("```").trim();
    }
    if trimmed.ends_with("```") {
        trimmed = trimmed.trim_end_matches("```").trim();
    }

    if let (Some(first), Some(last)) = (
        trimmed.find(|c| c == '{' || c == '['),
        trimmed.rfind(|c| c == '}' || c == ']'),
    ) {
        if last >= first {
            return &trimmed[first..=last];
        }
    }
    trimmed
}

/// Sanitasi string JSON terhadap karakter kontrol unescaped (RFC 8259: newline, carriage return, tab, ASCII < 0x20)
/// yang sering dihasilkan oleh LLM dalam teks multi-baris sehingga menyebabkan serde_json error.
pub fn sanitize_json_control_chars(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 64);
    let mut in_string = false;
    let mut escaped = false;

    for c in input.chars() {
        if !in_string {
            if c == '"' {
                in_string = true;
            }
            out.push(c);
        } else {
            if escaped {
                out.push(c);
                escaped = false;
            } else if c == '\\' {
                out.push(c);
                escaped = true;
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else if c == '\n' {
                out.push_str("\\n");
            } else if c == '\r' {
                out.push_str("\\r");
            } else if c == '\t' {
                out.push_str("\\t");
            } else if (c as u32) < 0x20 {
                out.push(' ');
            } else {
                out.push(c);
            }
        }
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. INFOGRAPHIC GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InfographicBlockDto {
    pub id: String,
    #[serde(rename = "type", alias = "block_type")]
    pub block_type: String, // "TEXT" | "IMAGE"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedInfographicDto {
    pub title: String,
    pub description: String,
    pub blocks: Vec<InfographicBlockDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_warning: Option<String>,
}

// ── Subject-Pedagogical Prompt Intelligence ──
pub fn is_english_subject(subject: &str) -> bool {
    let lower = subject.trim().to_lowercase();
    lower.contains("inggris") || lower.contains("english")
}

pub fn build_subject_pedagogical_instructions(subject: &str, grade: &str) -> String {
    if is_english_subject(subject) {
        return format!(
            r#"### MANDATORY SUBJECT RULE (ENGLISH LANGUAGE / {subject}):
- EVERY part of the generated output — title, description, instructions, rubric, headings, summaries, reading passages, dialogues, questions, interactive game items, options (A, B, C, D), and explanations — MUST BE WRITTEN 100% IN NATURAL ENGLISH.
- STRICTLY FORBIDDEN to use Indonesian words anywhere in student-facing content, headings, titles, descriptions, questions, or options.
- Calibrate vocabulary and grammar complexity appropriately for {grade}.
- Emphasize authentic communicative English skills (Reading Comprehension, Grammar in Context, Vocabulary Building, and relevant text types such as Narrative, Descriptive, Recount, Report, or Procedure)."#
        );
    }

    let lower_subject = subject.trim().to_lowercase();

    let specific_rule = if lower_subject.contains("matematika")
        || lower_subject.contains("math")
        || lower_subject.contains("aljabar")
        || lower_subject.contains("geometri")
        || lower_subject.contains("kalkulus")
    {
        format!(
            r#"### ATURAN WAJIB MAPEL (MATEMATIKA / MATHEMATICS):
- Matematika adalah ilmu eksak terapan, BUKAN esai naratif umum.
- Materi WAJIB menyertakan: rumus matematika yang jelas, angka konkret, persamaan matematis, dan contoh perhitungan langkah demi langkah (step-by-step).
- Soal kuis & tugas HARUS berupa soal perhitungan matematis nyata, pemecahan masalah (problem-solving), atau soal cerita kontekstual (word problems) sesuai level {grade}.
- Opsi jawaban A, B, C, D kuis CBT harus berupa nilai kuantitatif/angka/persamaan yang presisi. Bagian pembahasan (explanation) wajib menyertakan langkah hitung yang runtut."#
        )
    } else if lower_subject.contains("cirebon")
        || lower_subject.contains("daerah")
        || lower_subject.contains("jawa")
        || lower_subject.contains("sunda")
    {
        format!(
            r#"### ATURAN WAJIB MAPEL (BAHASA DAERAH / MUATAN LOKAL):
- Gunakan kosakata dan ragam bahasa daerah yang tepat ({subject}) serta nilai kearifan lokal (tradisi, seni, tatakrama/unggah-ungguh basa).
- Lengkapi dengan arti kata/glosarium kosakata daerah agar siswa {grade} dapat memahami dengan optimal."#
        )
    } else if lower_subject.contains("fisika")
        || lower_subject.contains("kimia")
        || lower_subject.contains("biologi")
        || lower_subject.contains("ipa")
        || lower_subject.contains("sains")
        || lower_subject.contains("science")
    {
        format!(
            r#"### ATURAN WAJIB MAPEL (IPA / SAINS / FISIKA / BIOLOGI / KIMIA):
- Gunakan prinsip ilmiah yang akurat: fenomena alam nyata, hukum fisika/reaksi kimia/proses biologis empiris, serta metode ilmiah.
- Jelaskan konsep sebab-akibat (kausalitas) dan aplikasi nyata dalam kehidupan sehari-hari siswa {grade}.
- Cantumkan istilah ilmiah yang baku beserta definisi/konteksnya yang mudah dimengerti."#
        )
    } else if lower_subject.contains("indonesia") {
        format!(
            r#"### ATURAN WAJIB MAPEL (BAHASA INDONESIA):
- Fokus pada keterampilan literasi berbahasa: pemahaman teks, kaidah kebahasaan (PUEBI/EYD), struktur jenis teks, majas, dan kosakata baku sesuai jenjang {grade}.
- Soal kuis & tugas harus menguji kemampuan analisis teks, ide pokok, kalimat efektif, dan penalaran kritis."#
        )
    } else if lower_subject.contains("agama")
        || lower_subject.contains("pai")
        || lower_subject.contains("akhlak")
    {
        format!(
            r#"### ATURAN WAJIB MAPEL (PENDIDIKAN AGAMA & BUDI PEKERTI):
- Tekankan nilai-nilai moral, akhlak mulia, dalil dan keteladanan yang relevan, serta pengamalan ibadah praktis dalam kehidupan sehari-hari siswa {grade}."#
        )
    } else if lower_subject.contains("sejarah")
        || lower_subject.contains("ips")
        || lower_subject.contains("geografi")
        || lower_subject.contains("ekonomi")
        || lower_subject.contains("sosiologi")
    {
        format!(
            r#"### ATURAN WAJIB MAPEL (IPS / SEJARAH / SOSIAL):
- Sertakan kronologi waktu peristiwa, tokoh penting, lokasi geografis, dan analisis dampak sosial-ekonomi nyata yang relevan bagi siswa {grade}."#
        )
    } else {
        format!(
            r#"### ATURAN MAPEL ({subject}):
- Sesuaikan materi, kedalaman konsep, dan bahasa pengantar secara ketat dengan karakteristik mata pelajaran {subject} pada jenjang {grade}."#
        )
    };

    format!(
        "{specific_rule}\n\n### ATURAN WAJIB MATA UANG INDONESIA (RUPIAH):\n- DALAM SEMUA SOAL (MATEMATIKA, EKONOMI, IPS, MAUPUN SOAL CERITA LAINNYA), JIKA TERDAPAT NOMINAL UANG, BIAYA, HARGA, ATAU ANGGARAN, WAJIB MENGGUNAKAN RUPIAH INDONESIA (\"Rp \" atau \"Rupiah\").\n- DILARANG KERAS MENGGUNAKAN SIMBOL ATAU MATA UANG DOLLAR ($) UNTUK NOMINAL UANG!\n- Gunakan nominal Rupiah yang realistis dan wajar di Indonesia (misal: \"Rp 10.000\", \"Rp 50.000\", \"Rp 100.000\", \"Rp 150.000\", \"Rp 1.000.000\").\n- Simbol tanda dollar ($) HANYA diizinkan untuk formula matematika LaTeX (misal: $x^2 + y = 10$). Untuk nominal uang, WAJIB tuliskan \"Rp \" bukan \"$\"."
    )
}

// ── Educational Visual Engine (Zero Inaccurate Unsplash Stock Photos) ──
pub fn get_safe_curated_educational_image(_subject: &str, _query: &str) -> String {
    String::new()
}

fn url_encode_wiki(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

async fn fetch_wikipedia_educational_image(query: &str) -> Option<String> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return None;
    }

    let client = get_ai_client();
    let langs = ["id", "en"];

    for lang in langs {
        // Step 1: Search Wikipedia for closest educational article title
        let search_url = format!(
            "https://{lang}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&utf8=1&srlimit=1",
            url_encode_wiki(clean_query)
        );

        let search_resp = match client
            .get(&search_url)
            .header("User-Agent", "SchoolOS-EducationPlatform/1.0 (admin@akselerasi.edu; child-safe-pedagogy)")
            .timeout(Duration::from_millis(2500))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };

        let search_json: serde_json::Value = match search_resp.json().await {
            Ok(j) => j,
            _ => continue,
        };

        let title = match search_json
            .get("query")
            .and_then(|q| q.get("search"))
            .and_then(|s| s.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("title"))
            .and_then(|t| t.as_str())
        {
            Some(t) if !t.trim().is_empty() => t,
            _ => continue,
        };

        // Step 2: Fetch page summary with image thumbnail/originalimage
        let summary_url = format!(
            "https://{lang}.wikipedia.org/api/rest_v1/page/summary/{}",
            url_encode_wiki(title)
        );

        let summary_resp = match client
            .get(&summary_url)
            .header("User-Agent", "SchoolOS-EducationPlatform/1.0 (admin@akselerasi.edu; child-safe-pedagogy)")
            .timeout(Duration::from_millis(2500))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };

        let summary_json: serde_json::Value = match summary_resp.json().await {
            Ok(j) => j,
            _ => continue,
        };

        let image_url = summary_json
            .get("thumbnail")
            .and_then(|img| img.get("source"))
            .and_then(|s| s.as_str())
            .or_else(|| {
                summary_json
                    .get("originalimage")
                    .and_then(|img| img.get("source"))
                    .and_then(|s| s.as_str())
            });

        if let Some(url) = image_url {
            let lower = url.to_lowercase();
            if lower.starts_with("http")
                && (lower.contains(".jpg")
                    || lower.contains(".jpeg")
                    || lower.contains(".png")
                    || lower.contains(".svg")
                    || lower.contains(".webp"))
            {
                tracing::info!(
                    "Found Wikipedia educational image for '{}' on [{}]: {}",
                    clean_query,
                    lang,
                    url
                );
                return Some(url.to_string());
            }
        }
    }

    None
}

pub async fn search_real_educational_image(query: &str, subject_name: &str) -> String {
    // 1. Query Wikipedia (Indonesian, then English) for actual encyclopedia diagram/photo
    if let Some(wiki_url) = fetch_wikipedia_educational_image(query).await {
        return wiki_url;
    }

    // 2. Query combined subject and topic if query was very concise
    if !query.trim().is_empty() && query.trim() != subject_name.trim() {
        let combined = format!("{} {}", subject_name, query);
        if let Some(wiki_url) = fetch_wikipedia_educational_image(&combined).await {
            return wiki_url;
        }
    }

    // 3. Fallback to child-safe curated educational catalog
    get_safe_curated_educational_image(subject_name, query)
}

#[derive(Deserialize)]
struct RawInfographicCard {
    step_number: u32,
    headline: String,
    summary: String,
    image_prompt: Option<String>,
    image_search_keyword: Option<String>,
    image_suggestion: Option<String>,
}

#[derive(Deserialize)]
struct RawInfographicResponse {
    title: String,
    description: String,
    cards: Vec<RawInfographicCard>,
}

async fn generate_infographic_images_batch(
    pool: &sqlx::PgPool,
    topic: &str,
    subject_name: &str,
    grade_level: &str,
    card_prompts: &[String],
) -> (Option<String>, Vec<Option<String>>, Option<String>) {
    use crate::infrastructure::cloudflare_ai::{generate_card_image, generate_hero_image};

    let hero_prompt = format!(
        "A rich wordless 3D Pixar-style visual scene depicting the core concept of {topic} in {subject_name} through expressive characters, symbolic objects, and vibrant scenery; pure clean illustration with no text, no typography, no words, no letters, and no banners"
    );

    let (hero_url, cf_cfg) = match generate_hero_image(pool, &hero_prompt, subject_name, grade_level).await {
        Ok((url, cfg)) => (Some(url), cfg),
        Err(warn_msg) => {
            return (None, vec![None; card_prompts.len()], Some(warn_msg));
        }
    };

    let fut_c0 = async {
        match card_prompts.first() {
            Some(p) => generate_card_image(pool, &cf_cfg, p, subject_name, grade_level).await,
            None => None,
        }
    };
    let fut_c1 = async {
        match card_prompts.get(1) {
            Some(p) => generate_card_image(pool, &cf_cfg, p, subject_name, grade_level).await,
            None => None,
        }
    };

    let (c0, c1) = tokio::join!(fut_c0, fut_c1);

    let fut_c2 = async {
        match card_prompts.get(2) {
            Some(p) => generate_card_image(pool, &cf_cfg, p, subject_name, grade_level).await,
            None => None,
        }
    };
    let fut_c3 = async {
        match card_prompts.get(3) {
            Some(p) => generate_card_image(pool, &cf_cfg, p, subject_name, grade_level).await,
            None => None,
        }
    };

    let (c2, c3) = tokio::join!(fut_c2, fut_c3);
    (hero_url, vec![c0, c1, c2, c3], None)
}

pub async fn generate_infographic(
    pool: &sqlx::PgPool,
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    include_images: bool,
) -> Result<GeneratedInfographicDto, ApplicationError> {
    let is_english = is_english_subject(subject_name);
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let prompt = if is_english {
        format!(
            r#"You are an educational visual media designer creating an INTERACTIVE INFOGRAPHIC module for {grade_level} students studying "{subject_name}" on the topic: "{topic}".

{subject_guideline}

Output MUST be strictly valid JSON using this exact schema (with EVERY value — title, description, headline, and summary — written 100% in natural English, NO Indonesian words):
{{
  "title": "Engaging Infographic Title on {topic}",
  "description": "Concise 1-2 sentence overview of this English visual learning module.",
  "cards": [
    {{
      "step_number": 1,
      "headline": "Introduction & Core Vocabulary",
      "summary": "Clear 2-3 sentence student-friendly explanation in natural English with relevant examples.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO posters, NO signs, NO captions, pure clean illustration)"
    }},
    {{
      "step_number": 2,
      "headline": "Key Concepts & Expressions",
      "summary": "Clear 2-3 sentence student-friendly explanation in natural English showing sentence patterns or grammar in context.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }},
    {{
      "step_number": 3,
      "headline": "Real-Life Usage & Dialogue",
      "summary": "Clear 2-3 sentence student-friendly explanation in natural English showing practical usage.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }},
    {{
      "step_number": 4,
      "headline": "Conclusion & Did You Know?",
      "summary": "Interesting insight and key takeaway written 100% in English.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }}
  ]
}}

SPECIAL RULE for image_prompt: Describe ONLY concrete visual objects, characters, expressions, and environment in English. NEVER ask for posters, chalkboards with writing, banners, speech bubbles, labels, or any text inside the illustration.
Output ONLY pure JSON with no leading or trailing prose."#
        )
    } else {
        format!(
            r#"Anda adalah pendidik spesialis penyusun media pembelajaran visual Kurikulum Merdeka di Indonesia.
Buatkan materi INFOGRAFIS INTERAKTIF untuk siswa jenjang {grade_level} pada mata pelajaran {subject_name} dengan topik: "{topic}".

{subject_guideline}

Format output HARUS berupa JSON murni dengan skema persis:
{{
  "title": "Judul Infografis Menarik",
  "description": "Deskripsi singkat modul 1-2 kalimat.",
  "cards": [
    {{
      "step_number": 1,
      "headline": "Judul Poin Visual 1",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mendalam dan mudah dipahami siswa.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO posters, NO signs, NO captions, pure clean illustration)"
    }},
    {{
      "step_number": 2,
      "headline": "Judul Poin Visual 2",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mendalam dan mudah dipahami siswa.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }},
    {{
      "step_number": 3,
      "headline": "Judul Poin Visual 3",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mendalam dan mudah dipahami siswa.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }},
    {{
      "step_number": 4,
      "headline": "Kesimpulan & Tahukah Kamu?",
      "summary": "Fakta menarik dan intisari penting untuk diingat siswa.",
      "image_prompt": "Detailed wordless visual scene in English describing ONLY physical objects, characters, actions, and setting (strictly NO text, NO words, NO letters, NO labels, pure clean illustration)"
    }}
  ]
}}

ATURAN KHUSUS image_prompt: Deskripsikan HANYA objek visual nyata, karakter, ekspresi, dan suasana alam/ruang secara konkret dalam bahasa Inggris. DILARANG meminta poster, papan tulis bertulisan, spanduk, balon kata, label, atau teks apa pun di dalam gambar agar ilustrasi bersih (no text, no typography, no words, no letters, pure clean illustration).
Output HANYA JSON tanpa teks pembuka/penutup."#
        )
    };

    let system_prompt = if is_english {
        "You are an English curriculum visual designer creating structured English infographic cards in strict JSON. Every title, description, headline, and summary MUST be written 100% in natural English (zero Indonesian). For every image_prompt field, write a concrete wordless English visual scene with no text, no typography, no words, no letters, and no labels."
    } else {
        "You are an educational designer creating structured infographic cards in strict JSON, strictly respecting the subject matter rules. For every image_prompt field, write a concrete wordless English visual scene with no text, no typography, no words, no letters, and no labels."
    };

    let messages = vec![
        ChatMessage {
            role: "system",
            content: system_prompt,
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = match call_nvidia_nim(messages, 0.3, 1800).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for infographic: {e}. Utilizing pedagogical fallback.");
            return Ok(fallback_generate_infographic(pool, topic, grade_level, subject_name, include_images).await);
        }
    };
    let cleaned = extract_clean_json(&raw);

    let parsed: RawInfographicResponse = match serde_json::from_str(cleaned) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse AI Infographic JSON ({e}). Utilizing pedagogical fallback.");
            return Ok(fallback_generate_infographic(pool, topic, grade_level, subject_name, include_images).await);
        }
    };

    let now_ts = chrono::Utc::now().timestamp_millis();
    let mut blocks = Vec::new();

    let card_prompts: Vec<String> = parsed
        .cards
        .iter()
        .take(4)
        .map(|c| {
            c.image_prompt
                .as_deref()
                .or(c.image_search_keyword.as_deref())
                .or(c.image_suggestion.as_deref())
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("{}: {} ({})", c.headline, s, topic))
                .unwrap_or_else(|| format!("{}: {} ({})", c.headline, c.summary, topic))
        })
        .collect();

    let (hero_url, card_urls, image_warning) = if include_images {
        generate_infographic_images_batch(pool, topic, subject_name, grade_level, &card_prompts).await
    } else {
        (None, vec![None; card_prompts.len()], None)
    };

    if let Some(cover_url) = hero_url {
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-hero-cover-{}", now_ts),
            block_type: "IMAGE".to_string(),
            content: cover_url,
        });
    }

    for (idx, card) in parsed.cards.into_iter().enumerate() {
        if let Some(Some(image_url)) = card_urls.get(idx) {
            if !image_url.trim().is_empty() {
                blocks.push(InfographicBlockDto {
                    id: format!("ai-block-img-{}-{}", idx + 1, now_ts),
                    block_type: "IMAGE".to_string(),
                    content: image_url.clone(),
                });
            }
        }

        blocks.push(InfographicBlockDto {
            id: format!("ai-block-text-{}-{}", idx + 1, now_ts),
            block_type: "TEXT".to_string(),
            content: format!(
                "### 📌 {}. {}\n\n{}",
                card.step_number, card.headline, card.summary
            ),
        });
    }

    Ok(GeneratedInfographicDto {
        title: parsed.title,
        description: parsed.description,
        blocks,
        image_warning,
    })
}

pub async fn fallback_generate_infographic(
    pool: &sqlx::PgPool,
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    include_images: bool,
) -> GeneratedInfographicDto {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let is_english = is_english_subject(subject_name);
    let mut blocks = Vec::new();

    let cards = if is_english {
        [
            (
                1,
                format!("Introduction to {topic}"),
                format!("Learning the fundamentals and core vocabulary of {topic} in {subject_name} ({grade_level}) to build confident English understanding."),
            ),
            (
                2,
                "Core Concepts & Language Patterns".to_string(),
                format!("Key concepts in {topic} connect essential vocabulary, sentence structures, and everyday communication contexts."),
            ),
            (
                3,
                "Real-Life Usage & Examples".to_string(),
                format!("In daily communication, expressions and examples around {topic} help students describe situations, share ideas, and interact clearly in English."),
            ),
            (
                4,
                "Summary & Did You Know?".to_string(),
                format!("Mastering {topic} strengthens reading comprehension, vocabulary range, and confident English communication."),
            ),
        ]
    } else {
        [
            (
                1,
                format!("Pengenalan {topic}"),
                format!("Mempelajari dasar-dasar {topic} pada mata pelajaran {subject_name} ({grade_level}) guna membangun pemahaman konseptual yang kokoh."),
            ),
            (
                2,
                "Prinsip Utama & Karakteristik".to_string(),
                format!("Konsep {topic} memiliki karakteristik spesifik yang menghubungkan teori dasar dengan fenomena yang terjadi di lingkungan sekitar."),
            ),
            (
                3,
                "Aplikasi & Penerapan Nyata".to_string(),
                format!("Dalam kehidupan sehari-hari, prinsip {topic} diterapkan untuk menyelesaikan berbagai tantangan dan masalah kontekstual secara analitis."),
            ),
            (
                4,
                "Kesimpulan & Refleksi Belajar".to_string(),
                format!("Penguasaan materi {topic} membentuk kecakapan bernalar kritis sesuai capaian pembelajaran Kurikulum Merdeka."),
            ),
        ]
    };

    let card_prompts: Vec<String> = cards
        .iter()
        .map(|(_, h, s)| format!("{h}: {s} ({topic})"))
        .collect();

    let (hero_url, card_urls, image_warning) = if include_images {
        generate_infographic_images_batch(pool, topic, subject_name, grade_level, &card_prompts).await
    } else {
        (None, vec![None; 4], None)
    };

    if let Some(cover_url) = hero_url {
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-hero-cover-{}", now_ts),
            block_type: "IMAGE".to_string(),
            content: cover_url,
        });
    }

    for (idx, (step_no, headline, summary)) in cards.into_iter().enumerate() {
        if let Some(Some(img_url)) = card_urls.get(idx) {
            if !img_url.trim().is_empty() {
                blocks.push(InfographicBlockDto {
                    id: format!("ai-block-img-{}-{}", step_no, now_ts),
                    block_type: "IMAGE".to_string(),
                    content: img_url.clone(),
                });
            }
        }
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-text-{}-{}", step_no, now_ts),
            block_type: "TEXT".to_string(),
            content: format!("### 📌 {}. {}\n\n{}", step_no, headline, summary),
        });
    }

    GeneratedInfographicDto {
        title: if is_english {
            format!("Module Infographic: {topic}")
        } else {
            format!("Infografis Modul: {topic}")
        },
        description: if is_english {
            format!("Interactive English visual learning module for {grade_level} ({subject_name}).")
        } else {
            format!("Media pembelajaran visual Kurikulum Merdeka jenjang {grade_level} ({subject_name}).")
        },
        blocks,
        image_warning,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. ARTICLE GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedArticleDto {
    pub title: String,
    pub description: String,
    pub article_content: String,
}

#[derive(Deserialize)]
struct RawArticleResponse {
    title: String,
    description: String,
    markdown_article: String,
}

fn extract_json_string_until_closing_quote(after_opening_quote: &str) -> String {
    let mut out = String::with_capacity(after_opening_quote.len());
    let mut chars = after_opening_quote.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => {}
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => break,
            }
        } else if c == '"' {
            // Stop cleanly at the unescaped closing quote of this JSON string
            break;
        } else {
            out.push(c);
        }
    }
    out
}

fn sanitize_article_markdown(raw_md: &str) -> String {
    let mut text = raw_md.replace("\r\n", "\n").trim().to_string();
    // Strip accidental leaked sibling JSON keys if present at tail
    for marker in [
        "\", \"glossary\"",
        "\",\"glossary\"",
        "\",\n  \"glossary\"",
        "\",\n\"glossary\"",
        "\", \"soal_essay\"",
        "\",\"soal_essay\"",
        "\",\n  \"soal_essay\"",
        "\",\n\"soal_essay\"",
        "\", \"soal_pilihan_ganda\"",
        "\",\"soal_pilihan_ganda\"",
        "\",\n  \"soal_pilihan_ganda\"",
        "\",\n\"soal_pilihan_ganda\"",
        "\", \"questions\"",
        "\",\"questions\"",
        "\",\n  \"questions\"",
        "\",\n\"questions\"",
        "\",\n  \"quiz\"",
        "\",\n\"quiz\"",
    ] {
        if let Some(pos) = text.find(marker) {
            text.truncate(pos);
        }
    }
    // Catch any generic `",` line followed by `"<json_key>":`
    let mut byte_offset = 0usize;
    let lines_snapshot: Vec<&str> = text.split('\n').collect();
    for i in 0..lines_snapshot.len().saturating_sub(1) {
        let cur = lines_snapshot[i].trim();
        let next = lines_snapshot[i + 1].trim();
        let is_json_key_line = next.starts_with('"') && next.contains("\":");
        if (cur == "\"," || cur == "\" ,") && is_json_key_line {
            text.truncate(byte_offset);
            break;
        }
        byte_offset += lines_snapshot[i].len() + 1;
    }
    text = text.trim_end_matches(|c| c == '}' || c == ',' || c == ' ' || c == '"').trim().to_string();

    // Expand collapsed single-line Markdown tables (`| Col | | :--- | | Val |`) onto separate lines
    // and repair unclosed trailing `**` on standalone heading/label lines
    let mut out_lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.contains("| :---") || line.contains("|:---") || line.contains("| ---") || line.contains("|---") {
            let expanded = line.replace("| |", "|\n|");
            for sub in expanded.lines() {
                let s = sub.trim();
                if !s.is_empty() {
                    out_lines.push(s.to_string());
                }
            }
        } else {
            let mut repaired = line.to_string();
            let trimmed = repaired.trim();
            if trimmed.matches("**").count() % 2 == 1 && trimmed.starts_with("**") && !trimmed[2..].contains('*') {
                repaired.push_str("**");
            }
            out_lines.push(repaired);
        }
    }
    out_lines.join("\n")
}

fn parse_or_extract_article(
    raw: &str,
    default_topic: &str,
    default_subject: &str,
) -> GeneratedArticleDto {
    let cleaned = extract_clean_json(raw);
    if let Ok(parsed) = serde_json::from_str::<RawArticleResponse>(cleaned) {
        return GeneratedArticleDto {
            title: parsed.title,
            description: parsed.description,
            article_content: sanitize_article_markdown(&parsed.markdown_article),
        };
    }

    // Jika parse langsung gagal (misal EOF parsing string karena token terpotong):
    // Coba tutup string dan JSON object
    let mut repaired = cleaned.to_string();
    if !repaired.ends_with('}') {
        repaired.push_str("\"}");
        if let Ok(parsed) = serde_json::from_str::<RawArticleResponse>(&repaired) {
            return GeneratedArticleDto {
                title: parsed.title,
                description: parsed.description,
                article_content: sanitize_article_markdown(&parsed.markdown_article),
            };
        }
    }

    // Fallback: Ekstraksi manual aman dengan scanning escape-sequence JSON (anti bocor sibling field)
    let title = if let Some(idx) = raw.find("\"title\"") {
        let rest = &raw[idx + 7..];
        if let Some(start) = rest.find('"') {
            let extracted = extract_json_string_until_closing_quote(&rest[start + 1..]);
            if extracted.trim().is_empty() {
                default_topic.to_string()
            } else {
                extracted
            }
        } else {
            default_topic.to_string()
        }
    } else {
        default_topic.to_string()
    };

    let description = if let Some(idx) = raw.find("\"description\"") {
        let rest = &raw[idx + 13..];
        if let Some(start) = rest.find('"') {
            let extracted = extract_json_string_until_closing_quote(&rest[start + 1..]);
            if !extracted.trim().is_empty() {
                extracted
            } else if is_english_subject(default_subject) {
                format!("Educational learning article for {default_subject}")
            } else {
                format!("Naskah artikel pembelajaran {default_subject}")
            }
        } else if is_english_subject(default_subject) {
            format!("Educational learning article for {default_subject}")
        } else {
            format!("Naskah artikel pembelajaran {default_subject}")
        }
    } else if is_english_subject(default_subject) {
        format!("Educational learning article for {default_subject}")
    } else {
        format!("Naskah artikel pembelajaran {default_subject}")
    };

    let raw_article = if let Some(idx) = raw.find("\"markdown_article\"") {
        let rest = &raw[idx + 18..];
        if let Some(start) = rest.find('"') {
            extract_json_string_until_closing_quote(&rest[start + 1..])
        } else {
            raw.to_string()
        }
    } else {
        raw.to_string()
    };

    GeneratedArticleDto {
        title,
        description,
        article_content: sanitize_article_markdown(&raw_article),
    }
}

pub async fn generate_article(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> Result<GeneratedArticleDto, ApplicationError> {
    let is_english = is_english_subject(subject_name);
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let prompt = if is_english {
        format!(
            r#"You are an expert English curriculum author.
Write a complete LEARNING ARTICLE for {grade_level} students studying "{subject_name}" on the topic: "{topic}".

{subject_guideline}

Output MUST be strictly valid JSON with ONLY these 3 keys: "title", "description", "markdown_article" (do NOT add separate JSON keys like "glossary", "soal_essay", "soal_pilihan_ganda", or "questions" anywhere inside or outside "markdown_article"). If you include Markdown tables, put EACH row on its own line separated by \n.
Every field — title, description, and the entire markdown_article — MUST be written 100% in natural English (NO Indonesian words):
{{
  "title": "Engaging English Learning Article Title on {topic}",
  "description": "Concise 1-2 sentence introduction to this learning module.",
  "markdown_article": "Full structured learning article in Markdown:\n# Title\n## 1. Introduction & Warm-Up\n## 2. Core Concepts & Language Focus (include clear rules, examples, or an authentic English reading passage/dialogue)\n## 3. Contextual Examples & Guided Practice\n## 4. Key Takeaways\n## 5. Vocabulary Glossary"
}}

Make sure the writing is thorough, clear, well-escaped valid JSON with properly closed quotes and braces. Output ONLY JSON with no other text."#
        )
    } else {
        format!(
            r#"Anda adalah guru ahli Kurikulum Merdeka di Indonesia.
Tuliskan naskah ARTIKEL PEMBELAJARAN lengkap untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}".

{subject_guideline}

Format output HARUS berupa JSON murni dengan HANYA 3 key ini saja: "title", "description", dan "markdown_article" (DILARANG menambahkan key JSON tambahan seperti "glossary", "soal_essay", "soal_pilihan_ganda", atau "questions" di luar maupun di dalam "markdown_article"; masukkan glosarium langsung sebagai teks Markdown di dalam markdown_article). Jika membuat tabel perbandingan Markdown, tuliskan SETIAP baris tabel di baris baru yang dipisah \n (bukan digabung dalam satu baris).
{{
  "title": "Judul Naskah Pembelajaran",
  "description": "Deskripsi pengantar singkat 1-2 kalimat.",
  "markdown_article": "Isi lengkap artikel berformat Markdown terstruktur:\n# Judul\n## 1. Pengantar & Apersepsi Menarik\n## 2. Pembahasan Konsep Inti (Disertai contoh nyata, rumus bila eksak, atau bacaan bahasa)\n## 3. Contoh Kasus & Aplikasi Nyata / Langkah Perhitungan\n## 4. Rangkuman Intisari\n## 5. Glosarium / Kamus Kata Kunci"
}}

Pastikan teks padat, mendalam, format JSON valid dengan escape karakter benar, dan selalu ditutup sempurna. Output HANYA JSON tanpa teks lain."#
        )
    };

    let system_prompt = if is_english {
        "You are an expert English educational content writer creating deep, engaging learning articles in strict JSON. Write 100% in natural English with zero Indonesian."
    } else {
        "You are an expert educational content writer creating deep, subject-specific learning articles in strict JSON."
    };

    let messages = vec![
        ChatMessage {
            role: "system",
            content: system_prompt,
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = match call_nvidia_nim(messages, 0.4, 4000).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for article: {e}. Utilizing pedagogical fallback.");
            return Ok(fallback_generate_article(topic, grade_level, subject_name));
        }
    };
    let result = parse_or_extract_article(&raw, topic, subject_name);

    Ok(result)
}

pub fn fallback_generate_article(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> GeneratedArticleDto {
    if is_english_subject(subject_name) {
        let article_md = format!(
            r#"# Learning Module: {topic}
**Subject:** {subject_name} | **Grade Level:** {grade_level} | **Curriculum:** Kurikulum Merdeka

---

## 1. Introduction & Warm-Up
Have you ever noticed how **{topic}** appears in everyday English conversations and reading texts? Understanding this topic helps students build stronger reading comprehension, vocabulary range, and confident communication skills in real-life contexts for {grade_level}.

## 2. Core Concepts & Language Focus
In studying **{topic}** ({subject_name}), focus on these three essential pillars:
1. **Key Vocabulary & Expressions:** Recognize important words, phrases, and expressions related to {topic} accurately in context.
2. **Sentence Structure & Grammar Patterns:** Observe how sentences around {topic} are constructed clearly and naturally in spoken and written English.
3. **Communicative Purpose:** Understand how speakers and writers use {topic} to share ideas, describe people or activities, and ask or answer questions.

> 💡 **Learning Tip:** Meaningful English mastery comes from understanding how words and expressions work in real communication, not just memorizing isolated lists.

## 3. Contextual Examples & Guided Analysis
Apply your understanding through a structured approach:
- **Context:** Reading a short dialogue or descriptive text focused on {topic}.
- **Step-by-Step Practice:**
  1. *Observe:* Identify the key words, main idea, and supporting details about {topic}.
  2. *Connect:* Relate the expressions to real situations in your daily routine at home or school.
  3. *Produce:* Write or speak two to three clear English sentences using the expressions you just learned.

## 4. Key Takeaways
- **{topic}** is a core communicative topic in {subject_name} for {grade_level}.
- Combining vocabulary recognition with authentic sentence practice leads to lasting fluency.
- Reviewing examples and practicing aloud strengthens both accuracy and confidence.

## 5. Key Vocabulary & Self-Reflection
- **{topic}:** The central theme integrating vocabulary, grammar in context, and everyday English communication.
- **Contextual Clue:** Information in a surrounding sentence or passage that helps clarify an unfamiliar word's meaning.
- **Self-Reflection:** Which new English words or expressions from "{topic}" can you start using in a conversation today?
"#
        );

        return GeneratedArticleDto {
            title: format!("Learning Module: {topic}"),
            description: format!("Structured English learning module for {grade_level} ({subject_name})."),
            article_content: article_md,
        };
    }

    let article_md = format!(
        r#"# Modul Pembelajaran: {topic}
**Mata Pelajaran:** {subject_name} | **Jenjang:** {grade_level} | **Standar:** Kurikulum Merdeka

---

## 1. Pengantar & Apersepsi
Pernahkah Anda memperhatikan bagaimana konsep **{topic}** bekerja dan diterapkan dalam kehidupan sehari-hari? Memahami konsep ini memberikan landasan berpikir analitis dan kritis dalam memecahkan masalah kontekstual. Modul ini disusun untuk membimbing siswa {grade_level} dalam memahami konsep secara bertahap, mendalam, dan aplikatif.

## 2. Pemahaman Konsep Inti
Secara mendasar, kajian **{topic}** dalam mata pelajaran {subject_name} mencakup pilar-pilar penting sebagai berikut:
1. **Landasan Teoretis:** Pemahaman definisi, kaidah dasar, dan batasan ruang lingkup materi.
2. **Karakteristik & Hubungan Antar-Konsep:** Bagaimana unsur-unsur di dalamnya saling mempengaruhi dan membentuk kesatuan sistem.
3. **Mekanisme & Kaidah Operasional:** Langkah runtut dan pembuktian logis yang melandasi fenomena atau perhitungan terkait.

> 💡 **Kunci Pedagogis Kurikulum Merdeka:** Belajar bermakna berakar pada pemahaman konseptual dan penalaran logis, bukan sekadar menghafal rumus atau istilah tanpa makna.

## 3. Contoh Terapan & Analisis Kasus
Untuk memperkuat pemahaman aplikatif, perhatikan skenario berikut:
- **Konteks Masalah:** Penerapan nyata prinsip {topic} dalam lingkungan akademik, sosial, atau teknologi di sekitar kita.
- **Langkah Pemecahan Sistematis:**
  1. *Identifikasi:* Telaah parameter utama dan informasi penting yang diketahui.
  2. *Analisis:* Gunakan konsep {subject_name} yang relevan untuk merancang alur pemecahan.
  3. *Verifikasi:* Uji kembali keabsahan hasil atau kesimpulan agar tepat sasaran.

## 4. Rangkuman Intisari
- **{topic}** adalah kompetensi krusial pada mata pelajaran {subject_name} jenjang {grade_level}.
- Penguasaan konsep ini melatih kemandirian, nalar kritis, dan keterampilan memecahkan masalah kompleks.
- Kolaborasi antara teori terstruktur dan studi kasus kontekstual menghasilkan pemahaman yang mendalam dan berdaya guna.

## 5. Glosarium & Refleksi Mandiri
- **{topic}:** Tema sentral pembelajaran yang mengintegrasikan kompetensi pengetahuan dan keterampilan.
- **Pendekatan Saintifik:** Pola berpikir sistematis berbasis pengamatan, penalaran, dan evaluasi logis.
- **Refleksi Pembelajaran:** Apa wawasan paling berharga yang Anda peroleh dari topik ini, dan bagaimana Anda dapat menerapkannya dalam memecahkan persoalan nyata?
"#
    );

    GeneratedArticleDto {
        title: format!("Materi Pembelajaran: {topic}"),
        description: format!("Naskah modul belajar terstruktur Kurikulum Merdeka untuk {subject_name} ({grade_level})."),
        article_content: article_md,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// 3. SHARED QUESTION & ASSIGNMENT GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CbtQuestionChoiceDto {
    pub choice_text: String,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssignmentQuestionDto {
    pub id: String,
    pub question_text: String,
    pub question_type: String, // "MULTIPLE_CHOICE" | "ESSAY" | "MATCHING" | "ORDERING" | "FILL_BLANK" | "CROSSWORD"
    pub points: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default)]
    pub choices: Vec<CbtQuestionChoiceDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_config: Option<serde_json::Value>,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedAssignmentDto {
    pub title: String,
    pub instructions: String,
    pub assignment_type: String,
    pub rubric: String,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub questions: Vec<AssignmentQuestionDto>,
}

fn json_value_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| {
                let v_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                format!("{k}: {v_str}")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                _ => v.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
    }
}

pub fn sanitize_currency_to_idr(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 32);
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        if chars[i] == '$' {
            let mut j = i + 1;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            let digit_start = j;
            while j < n {
                if chars[j].is_ascii_digit() {
                    j += 1;
                } else if (chars[j] == '.' || chars[j] == ',') && (j + 1 < n && chars[j + 1].is_ascii_digit()) {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > digit_start {
                let is_math_var = if j < n {
                    let next_c = chars[j];
                    next_c.is_ascii_alphabetic() || next_c == '^' || next_c == '_'
                } else {
                    false
                };

                if !is_math_var {
                    let num_digits: String = chars[digit_start..j]
                        .iter()
                        .filter(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(mut val) = num_digits.parse::<i64>() {
                        if val < 1000 {
                            val *= 1000;
                        } else if val == 1000 {
                            val = 1_000_000;
                        }
                        let s = val.to_string();
                        let rem = s.len() % 3;
                        let mut formatted = String::with_capacity(s.len() + 4);
                        for (idx, ch) in s.chars().enumerate() {
                            if idx > 0 && (idx % 3 == rem || (rem == 0 && idx % 3 == 0)) {
                                formatted.push('.');
                            }
                            formatted.push(ch);
                        }
                        result.push_str("Rp ");
                        result.push_str(&formatted);
                        i = j;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

pub fn clean_choice_text(raw: &str) -> String {
    let sanitized = sanitize_currency_to_idr(raw);
    let trimmed = sanitized.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.chars().next().unwrap();
        if first.is_ascii_alphabetic() {
            let rest = &trimmed[first.len_utf8()..];
            let mut chars = rest.chars();
            if let Some(punct) = chars.next() {
                if punct == '.' || punct == ')' || punct == ':' || punct == '-' {
                    return chars.as_str().trim_start().to_string();
                }
            }
        }
    }
    trimmed.to_string()
}

pub fn clean_question_stem(raw: &str) -> String {
    let sanitized = sanitize_currency_to_idr(raw);
    let lines: Vec<&str> = sanitized.lines().collect();
    if lines.len() <= 1 {
        return sanitized.trim().to_string();
    }

    let mut cut_idx = lines.len();
    for i in (0..lines.len()).rev() {
        let line = lines[i].trim();
        if line.is_empty() {
            continue;
        }
        let is_choice_line = if line.len() >= 2 {
            let first = line.chars().next().unwrap();
            if first.is_ascii_alphabetic() {
                let rest = &line[first.len_utf8()..];
                if let Some(punct) = rest.chars().next() {
                    punct == '.' || punct == ')' || punct == ':' || punct == '-'
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if is_choice_line {
            cut_idx = i;
        } else {
            break;
        }
    }

    if cut_idx > 0 && cut_idx < lines.len() {
        lines[..cut_idx].join("\n").trim().to_string()
    } else {
        raw.trim().to_string()
    }
}

fn strip_leaked_diagram_bracket_tag(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let lower_prefixes = ["[diagram:", "[grafik:", "[bagan:", "[gambar:", "[chart:"];
    while let Some(open_idx) = rest.find('[') {
        out.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx..];
        let lower_slice: String = after_open.chars().take(12).collect::<String>().to_lowercase();
        if lower_prefixes.iter().any(|p| lower_slice.starts_with(p)) {
            if let Some(close_idx) = after_open.find(']') {
                rest = after_open[close_idx + 1..].trim_start();
                continue;
            }
        }
        out.push('[');
        rest = &rest[open_idx + 1..];
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn build_visual_diagram_prompt_instructions(is_english: bool, include_images: bool) -> &'static str {
    if !include_images {
        return "";
    }
    if is_english {
        r#"
VISUAL DIAGRAM / IMAGE QUESTION REQUIREMENT:
- On 1 to 2 non-game questions (prefer MULTIPLE_CHOICE or ESSAY), include a structured `"diagram_spec"` object AND explicitly refer to that exact diagram inside `"question_text"` (for example: "Observe the bar chart/diagram below...").
- NEVER write bracketed text placeholders like `[Diagram: ...]` or reveal the answer inside `"question_text"` — the system automatically renders `"diagram_spec"` as an actual visual image above the question.
- EVERY number, unit, vertex letter, and step label in `"question_text"` and options MUST match `"diagram_spec"` 100%. Never ask about details that are missing from `"diagram_spec"`.
- Allowed `"diagram_spec"` shapes:
  1) `{"type":"bar_chart","title":"...","x_label":"...","y_label":"...","bars":[{"label":"A","value":12},{"label":"B","value":18}]}`
  2) `{"type":"line_chart","title":"...","x_label":"...","y_label":"...","points":[{"label":"Mon","value":10},{"label":"Tue","value":16}]}`
  3) `{"type":"pie_chart","title":"...","slices":[{"label":"Math","value":40},{"label":"Science","value":60}]}`
  4) `{"type":"geometry","shape":"right_triangle|triangle|rectangle|square|circle|trapezoid|cube|cuboid|angle","title":"...","labels":["A","B","C"],"dimensions":{"a":"6 cm","b":"8 cm","c":"? cm"}}`
  5) `{"type":"flowchart","layout":"linear|cycle","title":"...","steps":[{"label":"1. Step","note":"..."},{"label":"2. ?","note":"..."}]}`
  6) `{"type":"science_diagram","subtype":"water_cycle|food_chain|circuit|lever|compass_map","title":"...","mode":"seri|paralel"}`
  7) `{"type":"illustration","prompt":"Wordless visual scene description for English picture comprehension"}`"#
    } else {
        r#"
ATURAN WAJIB SOAL BERGAMBAR / DIAGRAM AKURAT:
- Pada 1 hingga 2 butir soal non-game (utamakan MULTIPLE_CHOICE atau ESSAY), sertakan properti `"diagram_spec"` terstruktur DAN referensikan gambar tersebut di `"question_text"` (misal: "Perhatikan grafik/diagram/gambar di atas! ...").
- DILARANG KERAS menulis teks tag seperti `[Diagram: ...]` atau `[Grafik: ...]` serta dilarang membocorkan kunci jawaban di dalam `"question_text"`, karena sistem otomatis merender `"diagram_spec"` menjadi gambar SVG sungguhan di atas soal.
- SELURUH angka, satuan, huruf titik sudut, maupun tahap di dalam `"question_text"` dan pilihan jawaban WAJIB SAMA PERSIS dengan isi `"diagram_spec"`. DILARANG bertanya tentang data yang tidak tergambar.
- Pilihan format `"diagram_spec"` yang didukung sistem rendering presisi:
  1) `{"type":"bar_chart","title":"...","x_label":"...","y_label":"...","bars":[{"label":"Senin","value":15},{"label":"Selasa","value":25}]}`
  2) `{"type":"line_chart","title":"...","x_label":"...","y_label":"...","points":[{"label":"Jan","value":10},{"label":"Feb","value":20}]}`
  3) `{"type":"pie_chart","title":"...","slices":[{"label":"IPA","value":40},{"label":"IPS","value":60}]}`
  4) `{"type":"geometry","shape":"right_triangle|triangle|rectangle|square|circle|trapezoid|cube|cuboid|angle","title":"...","labels":["A","B","C"],"dimensions":{"a":"6 cm","b":"8 cm","c":"? cm"}}`
  5) `{"type":"flowchart","layout":"linear|cycle","title":"...","steps":[{"label":"1. Evaporasi","note":"Penguapan"},{"label":"2. ?","note":"Membentuk awan"}]}`
  6) `{"type":"science_diagram","subtype":"water_cycle|food_chain|circuit|lever|compass_map","title":"...","mode":"seri|paralel"}`"#
    }
}

pub async fn generate_assignment(
    pool: &sqlx::PgPool,
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    assignment_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    include_images: bool,
) -> Result<GeneratedAssignmentDto, ApplicationError> {
    let is_english = is_english_subject(subject_name);
    let visual_instructions = build_visual_diagram_prompt_instructions(is_english, include_images);
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let fmt_upper = assignment_format.unwrap_or("STRUCTURED_QUESTIONS").to_uppercase();
    let is_homework = fmt_upper == "HOMEWORK_PR";
    let is_pure_game = fmt_upper == "INTERACTIVE_GAMES";
    let is_game_mixed = fmt_upper.contains("GAME") && !is_pure_game;
    let effective_num_mcq = if is_homework || is_pure_game { 0 } else { num_mcq };
    let effective_num_essay = if is_homework {
        num_essay.max(1)
    } else if is_pure_game || is_game_mixed {
        0
    } else if effective_num_mcq == 0 && num_essay == 0 {
        2
    } else {
        num_essay
    };
    let now_ts = chrono::Utc::now().timestamp_millis();

    let num_games: usize = if is_pure_game || is_game_mixed { 4 } else { 0 };

    let total_questions = if is_homework {
        effective_num_essay
    } else if is_pure_game {
        num_games
    } else {
        effective_num_mcq + effective_num_essay + num_games
    };

    let game_schema_hint = if is_english {
        r#"
MANDATORY JSON EXAMPLES FOR THE 4 INTERACTIVE GAME TYPES (WRITE EVERY ITEM 100% IN ENGLISH):
1) Type 'MATCHING' (4 pairs):
{
  "id": "g-match-1",
  "question_text": "Match each English word, expression, or concept below with its accurate meaning:",
  "question_type": "MATCHING",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "pairs": [
      { "id": "p1", "left": "Term A", "right": "Clear English definition A" },
      { "id": "p2", "left": "Term B", "right": "Clear English definition B" },
      { "id": "p3", "left": "Term C", "right": "Clear English definition C" },
      { "id": "p4", "left": "Term D", "right": "Clear English definition D" }
    ]
  },
  "explanation": "Short English explanation..."
}
2) Type 'ORDERING' (4 steps in correct chronological/logical order from first to last):
{
  "id": "g-order-1",
  "question_text": "Arrange the following steps or dialogue lines into the correct order:",
  "question_type": "ORDERING",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "items": [
      { "id": "o1", "text": "Correct first step in English..." },
      { "id": "o2", "text": "Correct second step in English..." },
      { "id": "o3", "text": "Correct third step in English..." },
      { "id": "o4", "text": "Correct fourth step in English..." }
    ]
  },
  "explanation": "Reason for the sequence..."
}
3) Type 'FILL_BLANK' (Cloze sentence with {{1}}, {{2}} placeholders + distractors):
{
  "id": "g-fill-1",
  "question_text": "Complete the blanks in the English sentence using the correct words from the word bank:",
  "question_type": "FILL_BLANK",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "template": "Complete English sentence with blank {{1}} and continuation {{2}}.",
    "blanks": ["answer_1", "answer_2"],
    "distractors": ["distractor_1", "distractor_2"]
  },
  "explanation": "Brief grammar or vocabulary explanation..."
}
4) Type 'CROSSWORD' (4 single-word uppercase A-Z answers without spaces + short English clues sharing common letters):
{
  "id": "g-crossword-1",
  "question_text": "Complete the crossword puzzle below using key vocabulary from the lesson:",
  "question_type": "CROSSWORD",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "words": [
      { "answer": "GRAMMAR", "clue": "System and structure of a language" },
      { "answer": "READING", "clue": "Skill of understanding written passages" },
      { "answer": "MEANING", "clue": "Definition or sense of a word" },
      { "answer": "EXAMPLE", "clue": "Specific sample used to clarify a rule" }
    ]
  },
  "explanation": "Vocabulary summary..."
}"#
    } else {
        r#"
CONTOH FORMAT WAJIB UNTUK 4 TIPE GAME INTERAKTIF:
1) Tipe 'MATCHING' (Menjodohkan 4 pasangan):
{
  "id": "g-match-1",
  "question_text": "Jodohkan konsep berikut dengan penjelasan atau pasangannya yang tepat:",
  "question_type": "MATCHING",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "pairs": [
      { "id": "p1", "left": "Istilah A", "right": "Definisi A yang tepat" },
      { "id": "p2", "left": "Istilah B", "right": "Definisi B yang tepat" },
      { "id": "p3", "left": "Istilah C", "right": "Definisi C yang tepat" },
      { "id": "p4", "left": "Istilah D", "right": "Definisi D yang tepat" }
    ]
  },
  "explanation": "Pembahasan singkat..."
}
2) Tipe 'ORDERING' (Urutkan 4 langkah/kronologi secara berurutan dari awal hingga akhir):
{
  "id": "g-order-1",
  "question_text": "Susunlah tahapan atau kronologi berikut sesuai urutan yang benar:",
  "question_type": "ORDERING",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "items": [
      { "id": "o1", "text": "Tahap pertama yang benar..." },
      { "id": "o2", "text": "Tahap kedua yang benar..." },
      { "id": "o3", "text": "Tahap ketiga yang benar..." },
      { "id": "o4", "text": "Tahap keempat yang benar..." }
    ]
  },
  "explanation": "Alasan urutan..."
}
3) Tipe 'FILL_BLANK' (Kalimat rumpang dengan placeholder {{1}}, {{2}} + distraktor):
{
  "id": "g-fill-1",
  "question_text": "Lengkapi bagian kalimat yang kosong menggunakan pilihan bank kata yang tepat:",
  "question_type": "FILL_BLANK",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "template": "Kalimat edukatif lengkap dengan bagian rumpang {{1}} serta lanjutan {{2}}.",
    "blanks": ["jawaban_1", "jawaban_2"],
    "distractors": ["pengecoh_1", "pengecoh_2"]
  },
  "explanation": "Penjelasan jawaban..."
}
4) Tipe 'CROSSWORD' (Teka-Teki Silang 4 kata kunci tunggal A-Z tanpa spasi + petunjuk singkat yang saling beririsan huruf):
{
  "id": "g-crossword-1",
  "question_text": "Isilah Teka-Teki Silang (TTS) berikut sesuai petunjuk mendatar dan menurun:",
  "question_type": "CROSSWORD",
  "points": 20,
  "choices": [],
  "interaction_config": {
    "words": [
      { "answer": "EVAPORASI", "clue": "Proses penguapan air akibat panas matahari" },
      { "answer": "KONDENSASI", "clue": "Perubahan uap air menjadi titik air di awan" },
      { "answer": "PRESIPITASI", "clue": "Turunnya titik air hujan ke permukaan bumi" },
      { "answer": "SIKLUS", "clue": "Rangkaian proses alam yang berulang" }
    ]
  },
  "explanation": "Ringkasan istilah penting pada Teka-Teki Silang..."
}"#
    };

    let slot_instructions = if is_english {
        if is_homework {
            format!(
                "Array 'questions' MUST contain EXACTLY {effective_num_essay} ESSAY / WORKSHEET items in natural English.\n- Items 1 to {effective_num_essay}: Type 'ESSAY' (writing, reading analysis, or contextual English task)."
            )
        } else if is_pure_game {
            format!(
                "Array 'questions' MUST contain EXACTLY 4 INTERACTIVE GAME TASKS IN ENGLISH (NO MULTIPLE_CHOICE & NO ESSAY):\n\
                - Item 1: question_type = 'MATCHING' (4 pairs in English).\n\
                - Item 2: question_type = 'ORDERING' (4 ordered steps/lines in English).\n\
                - Item 3: question_type = 'FILL_BLANK' (template in English containing {{{{1}}}} and {{{{2}}}}, 2 blanks, 2 distractors).\n\
                - Item 4: question_type = 'CROSSWORD' (4 single uppercase A-Z words sharing common letters + clear English clues).\n\
                - STRICTLY FORBIDDEN to include 'ESSAY' or 'MULTIPLE_CHOICE' items!\n\
                {game_schema_hint}"
            )
        } else if is_game_mixed {
            format!(
                "Array 'questions' MUST contain EXACTLY {total_questions} AUTO-GRADED ENGLISH QUESTIONS (NO ESSAY):\n\
                - {effective_num_mcq} 'MULTIPLE_CHOICE' items (4 options, strictly 1 is_correct: true)\n\
                - 1 'MATCHING' item (4 pairs in English)\n\
                - 1 'ORDERING' item (4 ordered items in English)\n\
                - 1 'FILL_BLANK' item (English template containing {{{{1}}}} and {{{{2}}}}, 2 blanks, 2 distractors)\n\
                - 1 'CROSSWORD' item (4 single uppercase A-Z vocabulary words sharing letters + clear English clues)\n\
                - STRICTLY FORBIDDEN to include 'ESSAY' items in this game format!\n\
                {game_schema_hint}"
            )
        } else if effective_num_mcq > 0 && effective_num_essay == 0 {
            format!(
                "Array 'questions' MUST contain EXACTLY {effective_num_mcq} MULTIPLE_CHOICE items ('question_type': 'MULTIPLE_CHOICE') in natural English with 4 choices and strictly 1 true.\nDo NOT include 'ESSAY' items! Exact length = {effective_num_mcq}."
            )
        } else if effective_num_mcq == 0 && effective_num_essay > 0 {
            format!(
                "Array 'questions' MUST contain EXACTLY {effective_num_essay} ESSAY items ('question_type': 'ESSAY') in natural English with clear English scoring rubrics.\nDo NOT include 'MULTIPLE_CHOICE' items! Exact length = {effective_num_essay}."
            )
        } else {
            format!(
                "Array 'questions' MUST contain EXACTLY {total_questions} ENGLISH QUESTIONS:\n- Items 1 to {effective_num_mcq}: MULTIPLE_CHOICE ('question_type': 'MULTIPLE_CHOICE') with 4 English choices.\n- Items {essay_start} to {total_questions}: ESSAY ('question_type': 'ESSAY') with English prompts and rubrics.",
                essay_start = effective_num_mcq + 1
            )
        }
    } else if is_homework {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {effective_num_essay} BUTIR SOAL ESSAY / LEMBAR KERJA.\n- Butir 1 sampai {effective_num_essay}: Tipe 'ESSAY' (analitis / tugas observasi kontekstual)."
        )
    } else if is_pure_game {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT 4 BUTIR TUGAS GAME INTERAKTIF AKTIF (TANPA PG & TANPA ESSAY):\n\
            - Butir 1: question_type = 'MATCHING' (4 pasangan konsep).\n\
            - Butir 2: question_type = 'ORDERING' (4 langkah terurut dari awal hingga akhir).\n\
            - Butir 3: question_type = 'FILL_BLANK' (template berisi {{{{1}}}} dan {{{{2}}}}, 2 blanks, 2 distractors).\n\
            - Butir 4: question_type = 'CROSSWORD' (Teka-Teki Silang berisi 4 kata tunggal huruf kapital A-Z tanpa spasi yang beririsan huruf + clue singkat).\n\
            - DILARANG menyertakan soal 'ESSAY' maupun 'MULTIPLE_CHOICE'!\n\
            {game_schema_hint}"
        )
    } else if is_game_mixed {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {total_questions} BUTIR SOAL OTOMATIS (TANPA ESSAY):\n\
            - {effective_num_mcq} butir 'MULTIPLE_CHOICE' (4 opsi A-D, tepat 1 is_correct: true)\n\
            - 1 butir 'MATCHING' (4 pairs)\n\
            - 1 butir 'ORDERING' (4 items berurutan benar)\n\
            - 1 butir 'FILL_BLANK' (template berisi {{{{1}}}} dan {{{{2}}}}, 2 blanks, 2 distractors)\n\
            - 1 butir 'CROSSWORD' (4 kata kunci A-Z + petunjuk Teka-Teki Silang)\n\
            - DILARANG KERAS menyertakan soal bertipe 'ESSAY' pada format Game ini!\n\
            {game_schema_hint}"
        )
    } else if effective_num_mcq > 0 && effective_num_essay == 0 {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {effective_num_mcq} BUTIR SOAL PILIHAN GANDA MURNI ('question_type': 'MULTIPLE_CHOICE') dengan 4 pilihan A, B, C, D dan kunci is_correct tepat 1 true.\nDILARANG menyertakan soal bertipe 'ESSAY'! Jumlah soal di array questions harus PERSIS {effective_num_mcq} butir."
        )
    } else if effective_num_mcq == 0 && effective_num_essay > 0 {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {effective_num_essay} BUTIR SOAL URAIAN / ESSAY MURNI ('question_type': 'ESSAY') dengan rubrik pedoman penskoran komprehensif.\nDILARANG menyertakan soal bertipe 'MULTIPLE_CHOICE'! Jumlah soal di array questions harus PERSIS {effective_num_essay} butir."
        )
    } else {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {total_questions} BUTIR SOAL DENGAN PEMBAGIAN PERSIS:\n- Butir 1 sampai {effective_num_mcq}: SOAL PILIHAN GANDA ('question_type': 'MULTIPLE_CHOICE') dengan 4 pilihan A, B, C, D dan kunci is_correct tepat 1 true.\n- Butir {essay_start} sampai {total_questions}: SOAL URAIAN / ESSAY ('question_type': 'ESSAY') dengan rubrik pedoman penskoran komprehensif.\nDILARANG KERAS mengurangi jumlah soal! Jumlah soal di array questions harus PERSIS {total_questions} butir.",
            essay_start = effective_num_mcq + 1
        )
    };

    let assignment_type_val = if is_homework {
        "HOMEWORK_PR"
    } else {
        "STRUCTURED_QUESTIONS"
    };

    let prompt = if is_english {
        format!(
            r#"You are an expert English curriculum assessment designer.
Create a complete English student assignment package for subject: "{subject_name}", topic: "{topic}", grade level: "{grade_level}", difficulty: "{difficulty}" (Batch ID: {now_ts}).

{subject_guideline}

STRUCTURE AND QUESTION COUNT RULES:
{slot_instructions}
{visual_instructions}

Output MUST be strictly valid pure JSON using this schema (EVERY text field MUST be 100% in natural English, NO Indonesian words):
{{
  "title": "English Assignment: {topic}",
  "instructions": "Clear step-by-step instructions in English...",
  "assignment_type": "{assignment_type_val}",
  "rubric": "Grading Rubric: 0-100 scale based on accuracy, comprehension, and completeness.",
  "tasks": [
    "Task 1: Clear measurable English task...",
    "Task 2: Contextual English communication practice..."
  ],
  "questions": [
    {{
      "id": "q1",
      "question_text": "Multiple-choice question stem written in English...",
      "question_type": "MULTIPLE_CHOICE",
      "points": 10,
      "choices": [
        {{ "choice_text": "Option A in English", "is_correct": true }},
        {{ "choice_text": "Option B in English", "is_correct": false }},
        {{ "choice_text": "Option C in English", "is_correct": false }},
        {{ "choice_text": "Option D in English", "is_correct": false }}
      ],
      "explanation": "Concise English explanation of the answer key..."
    }},
    {{
      "id": "q2",
      "question_text": "Analytical or communicative English essay prompt...",
      "question_type": "ESSAY",
      "points": 20,
      "choices": [],
      "explanation": "English essay grading rubric..."
    }}
  ]
}}

STRICT REQUIREMENTS:
- Write 100% in natural English appropriate for {grade_level}. ZERO Indonesian words.
- 'question_text' must contain ONLY the question stem (no A/B/C/D options inside 'question_text').
- 'choice_text' must NOT include prefix labels like 'A.', 'B.', 'C.', 'D.'.
- 'questions' length MUST be EXACTLY {total_questions}.
- 'rubric' and 'instructions' MUST be single plain strings (NOT nested JSON objects).
- Output ONLY pure JSON with escaped newlines (\n) inside strings."#
        )
    } else {
        format!(
            r#"Anda adalah pengembang asesmen resmi Kurikulum Merdeka Indonesia.
Buatkan paket penugasan siswa terstruktur untuk mata pelajaran: "{subject_name}", topik: "{topic}", jenjang: "{grade_level}", tingkat kesulitan: "{difficulty}" (Batch ID: {now_ts}).

{subject_guideline}

ATURAN WAJIB MATA UANG INDONESIA (RUPIAH):
- Jika soal melibatkan uang, harga, biaya, modal, untung/rugi, atau anggaran, WAJIB menggunakan Rupiah ("Rp " atau "Rupiah", contoh: "Rp 10.000", "Rp 50.000", "Rp 100.000", "Rp 150.000", "Rp 1.000.000").
- DILARANG KERAS MENGGUNAKAN SIMBOL ATAU MATA UANG DOLLAR ($) UNTUK NOMINAL UANG!

ATURAN STRUKTUR DAN JUMLAH SOAL:
{slot_instructions}
{visual_instructions}

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Tugas Siswa: {topic}",
  "instructions": "Petunjuk pengerjaan penugasan langkah demi langkah...",
  "assignment_type": "{assignment_type_val}",
  "rubric": "Rubrik Penilaian: Skor 100 dengan kriteria penilaian...",
  "tasks": [
    "Tugas 1: Butir tugas operasional terukur...",
    "Tugas 2: Butir tugas pemecahan masalah / aplikasi nyata..."
  ],
  "questions": [
    {{
      "id": "q1",
      "question_text": "Teks butir soal pilihan ganda nomor 1...",
      "question_type": "MULTIPLE_CHOICE",
      "points": 10,
      "choices": [
        {{ "choice_text": "Pilihan A", "is_correct": true }},
        {{ "choice_text": "Pilihan B", "is_correct": false }},
        {{ "choice_text": "Pilihan C", "is_correct": false }},
        {{ "choice_text": "Pilihan D", "is_correct": false }}
      ],
      "explanation": "Pembahasan kunci jawaban..."
    }},
    {{
      "id": "q2",
      "question_text": "Teks butir soal uraian / essay analitis...",
      "question_type": "ESSAY",
      "points": 20,
      "choices": [],
      "explanation": "Rubrik pedoman penskoran essay..."
    }}
  ]
}}

PENTING & WAJIB:
- Pastikan teks soal dan pilihan jawaban SESUAI ATURAN MAPEL ({subject_name}).
- 'question_text' HANYA BERISI TEKS PERTANYAAN UTAMA / STIMULUS SOAL. DILARANG KERAS MENYISIPKAN OPSI PILIHAN JAWABAN (A, B, C, D) KE DALAM 'question_text'. Seluruh opsi pilihan HANYA boleh ada di dalam array 'choices'!
- Nilai 'choice_text' di dalam 'choices' HANYA berisi kalimat jawaban tanpa menyertakan label prefix 'A.', 'B.', 'C.', atau 'D.'.
- Array 'questions' HARUS berisi TEPAT {total_questions} butir soal sesuai instruksi komposisi di atas. Jangan berhenti menulis sebelum mencapai {total_questions} butir!
- Nilai 'rubric' dan 'instructions' HARUS berupa STRING TEKS tunggal (BUKAN JSON object/map).
- Output HANYA JSON tanpa teks lain.
- Pastikan seluruh string multi-baris menggunakan escape sequence \n dan BUKAN karakter raw newline."#
        )
    };

    let system_prompt = if is_english {
        "You are an expert English curriculum assessor creating 100% English student assignments and interactive learning games in strict JSON with zero Indonesian."
    } else {
        "You are an educational assessor creating student assignments strictly adhering to Indonesian Kurikulum Merdeka in strict JSON."
    };

    let messages = vec![
        ChatMessage {
            role: "system",
            content: system_prompt,
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = match call_nvidia_nim(messages, 0.65, 3800).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for assignment: {e}. Utilizing pedagogical fallback.");
            let mut dto = fallback_generate_assignment(
                topic,
                grade_level,
                subject_name,
                assignment_format,
                effective_num_mcq,
                effective_num_essay,
                difficulty,
                include_images,
            );
            if include_images {
                ensure_assignment_visual_guarantee(
                    pool,
                    &mut dto.questions,
                    subject_name,
                    topic,
                    grade_level,
                    is_english,
                )
                .await;
            }
            return Ok(dto);
        }
    };
    let cleaned = extract_clean_json(&raw);
    let sanitized = sanitize_json_control_chars(cleaned);

    #[derive(Deserialize)]
    struct RawAssignmentResponse {
        title: Option<serde_json::Value>,
        instructions: Option<serde_json::Value>,
        #[serde(default)]
        assignment_type: Option<String>,
        rubric: Option<serde_json::Value>,
        #[serde(default)]
        tasks: Option<serde_json::Value>,
        #[serde(default)]
        questions: Vec<serde_json::Value>,
    }

    let parsed: RawAssignmentResponse = match serde_json::from_str(&sanitized) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse AI Assignment JSON ({e}). Utilizing pedagogical fallback.");
            let mut dto = fallback_generate_assignment(
                topic,
                grade_level,
                subject_name,
                assignment_format,
                effective_num_mcq,
                effective_num_essay,
                difficulty,
                include_images,
            );
            if include_images {
                ensure_assignment_visual_guarantee(
                    pool,
                    &mut dto.questions,
                    subject_name,
                    topic,
                    grade_level,
                    is_english,
                )
                .await;
            }
            return Ok(dto);
        }
    };

    let mut diagrams_attached = 0usize;

    let title_str = parsed
        .title
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_english {
                format!("English Assignment: {topic}")
            } else {
                format!("Tugas Siswa: {topic}")
            }
        });
    let instructions_str = parsed
        .instructions
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_english {
                "Complete each task and question below carefully and thoughtfully in English.".to_string()
            } else {
                "Kerjakan penugasan berikut dengan seksama dan teliti.".to_string()
            }
        });
    let rubric_str = parsed.rubric.map(|v| json_value_to_string(&v)).filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_english {
                "Grading Rubric: 0 - 100 scale based on English comprehension, grammatical accuracy, and completeness.".to_string()
            } else {
                "Rubrik Penilaian: Skala Skor 0 - 100 berdasarkan ketepatan konsep dan kelengkapan analisis.".to_string()
            }
        });

    let tasks_vec: Vec<String> = match parsed.tasks {
        Some(serde_json::Value::Array(arr)) => {
            arr.into_iter().map(|v| json_value_to_string(&v)).collect()
        }
        Some(serde_json::Value::String(s)) => vec![s],
        _ => Vec::new(),
    };

    let mut questions: Vec<AssignmentQuestionDto> = Vec::new();

    for (idx, q_val) in parsed.questions.into_iter().enumerate() {
        let q_id = q_val
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("asg-q-{}", idx + 1));
        let q_text_raw = q_val
            .get("question_text")
            .or_else(|| q_val.get("text"))
            .map(json_value_to_string)
            .unwrap_or_default();
        let q_text = clean_question_stem(&q_text_raw);
        if q_text.trim().is_empty() {
            continue;
        }

        let mut choices: Vec<CbtQuestionChoiceDto> = Vec::new();
        if let Some(choices_val) = q_val.get("choices") {
            if let Some(c_arr) = choices_val.as_array() {
                for (c_idx, c_item) in c_arr.iter().enumerate() {
                    let c_text_raw = c_item
                        .get("choice_text")
                        .or_else(|| c_item.get("text"))
                        .map(json_value_to_string)
                        .unwrap_or_else(|| json_value_to_string(c_item));
                    let c_text = clean_choice_text(&c_text_raw);
                    let is_correct = c_item
                        .get("is_correct")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(c_idx == 0);
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: c_text,
                        is_correct,
                    });
                }
            }
        } else if let Some(options_val) = q_val.get("options") {
            if let Some(map) = options_val.as_object() {
                let correct_key = q_val
                    .get("correct_key")
                    .and_then(|k| k.as_str())
                    .unwrap_or("A");
                for (k, v) in map {
                    let raw_v = json_value_to_string(v);
                    let c_text = clean_choice_text(&raw_v);
                    let is_correct = k.trim().eq_ignore_ascii_case(correct_key.trim());
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: c_text,
                        is_correct,
                    });
                }
            }
        }

        let explicit_type = q_val.get("question_type").and_then(|v| v.as_str());
        let mut raw_interaction = q_val.get("interaction_config").cloned();
        let question_type = explicit_type.map(|t| t.trim().to_uppercase()).unwrap_or_else(|| {
            if raw_interaction.is_some() {
                if raw_interaction.as_ref().and_then(|v| v.get("pairs")).is_some() {
                    "MATCHING".to_string()
                } else if raw_interaction.as_ref().and_then(|v| v.get("items")).is_some() {
                    "ORDERING".to_string()
                } else if raw_interaction
                    .as_ref()
                    .and_then(|v| v.get("words").or_else(|| v.get("clues")))
                    .is_some()
                {
                    "CROSSWORD".to_string()
                } else {
                    "FILL_BLANK".to_string()
                }
            } else if choices.is_empty() {
                "ESSAY".to_string()
            } else {
                "MULTIPLE_CHOICE".to_string()
            }
        });

        if question_type == "FILL_BLANK" {
            if let Some(cfg) = raw_interaction.as_ref() {
                raw_interaction = Some(normalize_fill_blank_interaction_config(cfg, &q_text));
            }
        } else if question_type == "CROSSWORD" {
            if let Some(cfg) = raw_interaction.as_ref() {
                raw_interaction = Some(
                    crate::presentation::learning::assignments::interactive_game::compile_crossword_interaction_config(cfg),
                );
            }
        }

        let default_points = if question_type == "MULTIPLE_CHOICE" {
            10
        } else {
            20
        };
        let points = q_val
            .get("points")
            .and_then(|p| p.as_i64())
            .map(|p| p as i32)
            .unwrap_or(default_points);

        let explanation = q_val
            .get("explanation")
            .map(json_value_to_string)
            .unwrap_or_default();
        let img = if include_images && diagrams_attached < 3 {
            let resolved = crate::infrastructure::visual_diagram::resolve_visual_diagram_url(
                pool,
                q_val.get("diagram_spec"),
                subject_name,
                grade_level,
            )
            .await;
            if resolved.is_some() {
                diagrams_attached += 1;
            }
            resolved
        } else {
            None
        };
        let clean_q_text = if img.is_some() {
            strip_leaked_diagram_bracket_tag(&q_text)
        } else {
            q_text
        };

        questions.push(AssignmentQuestionDto {
            id: q_id,
            question_text: clean_q_text,
            question_type,
            points,
            image_url: img,
            choices,
            interaction_config: raw_interaction,
            explanation,
        });
    }

    // Fallback if questions array was empty but tasks existed
    if questions.is_empty() && !tasks_vec.is_empty() {
        for (idx, task_str) in tasks_vec.iter().enumerate() {
            questions.push(AssignmentQuestionDto {
                id: format!("asg-task-{}", idx + 1),
                question_text: task_str.clone(),
                question_type: "ESSAY".to_string(),
                points: 20,
                image_url: None,
                choices: Vec::new(),
                interaction_config: None,
                explanation: rubric_str.clone(),
            });
        }
    }

    // ── Enforce Exact Counts & Keep Interactive Game Questions ──
    if !is_homework {
        let mut final_mcqs: Vec<AssignmentQuestionDto> = Vec::new();
        let mut final_games: Vec<AssignmentQuestionDto> = Vec::new();
        let mut final_essays: Vec<AssignmentQuestionDto> = Vec::new();

        for q in questions {
            let qt = q.question_type.as_str();
            if qt == "MULTIPLE_CHOICE" && !q.choices.is_empty() {
                final_mcqs.push(q);
            } else if matches!(qt, "MATCHING" | "ORDERING" | "FILL_BLANK" | "CROSSWORD")
                && q.interaction_config
                    .as_ref()
                    .map(|cfg| is_valid_game_interaction_config(qt, cfg))
                    .unwrap_or(false)
            {
                final_games.push(q);
            } else {
                final_essays.push(q);
            }
        }

        if is_pure_game || is_game_mixed {
            if !final_games.iter().any(|g| g.question_type == "MATCHING") {
                final_games.push(build_fallback_matching_question(topic, subject_name, now_ts));
            }
            if !final_games.iter().any(|g| g.question_type == "ORDERING") {
                final_games.push(build_fallback_ordering_question(topic, subject_name, now_ts));
            }
            if !final_games.iter().any(|g| g.question_type == "FILL_BLANK") {
                final_games.push(build_fallback_fill_blank_question(topic, subject_name, now_ts));
            }
            if !final_games.iter().any(|g| g.question_type == "CROSSWORD") {
                final_games.push(build_fallback_crossword_question(topic, subject_name, now_ts));
            }
        }

        if is_pure_game {
            questions = final_games;
        } else {
            // Pad missing MCQs if AI returned fewer than requested
            if final_mcqs.len() < effective_num_mcq {
                let needed = effective_num_mcq - final_mcqs.len();
                let pad_templates = generate_pedagogical_question_templates(
                    subject_name,
                    topic,
                    grade_level,
                    difficulty,
                    effective_num_mcq,
                );
                let start_idx = final_mcqs.len();
                for offset in 0..needed {
                    let tpl = &pad_templates[(start_idx + offset) % pad_templates.len()];
                    let idx = final_mcqs.len() + 1;
                    let choices = tpl
                        .1
                        .iter()
                        .enumerate()
                        .map(|(c_idx, text)| CbtQuestionChoiceDto {
                            choice_text: text.clone(),
                            is_correct: c_idx == tpl.2,
                        })
                        .collect();
                    final_mcqs.push(AssignmentQuestionDto {
                        id: format!("asg-mcq-pad-{idx}-{now_ts}"),
                        question_text: tpl.0.clone(),
                        question_type: "MULTIPLE_CHOICE".to_string(),
                        points: 10,
                        image_url: None,
                        choices,
                        interaction_config: None,
                        explanation: tpl.3.clone(),
                    });
                }
            }

            // Pad missing Essays if AI returned fewer than requested (always 0 for game formats)
            while final_essays.len() < effective_num_essay {
                let idx = final_essays.len() + 1;
                final_essays.push(AssignmentQuestionDto {
                    id: format!("asg-essay-pad-{idx}-{now_ts}"),
                    question_text: if is_english {
                        format!("Explain your understanding and practical examples of '{topic}' in {subject_name} ({grade_level}) (Part {idx}):")
                    } else {
                        format!("Jelaskan secara mendalam analisis dan penerapan konsep {topic} dalam konteks nyata (Aspek Evaluasi {idx}):")
                    },
                    question_type: "ESSAY".to_string(),
                    points: 20,
                    image_url: None,
                    choices: Vec::new(),
                    interaction_config: None,
                    explanation: if is_english {
                        format!("Rubric: Full credit is awarded for accurate English explanations of {topic}, clear supporting examples, and coherent reasoning.")
                    } else {
                        format!("Rubrik: Skor maksimal jika siswa mampu menjabarkan konsep {topic} disertai contoh konkret dan argumentasi logis yang runtut.")
                    },
                });
            }

            final_mcqs.truncate(effective_num_mcq);
            final_essays.truncate(effective_num_essay);

            questions = final_mcqs;
            questions.extend(final_games);
            questions.extend(final_essays);
        }
    } else {
        while questions.len() < effective_num_essay {
            let idx = questions.len() + 1;
            questions.push(AssignmentQuestionDto {
                id: format!("asg-hw-pad-{idx}-{now_ts}"),
                question_text: if is_english {
                    format!("Independent Task {idx}: Observe real-life examples and write a structured report in English about '{topic}' for {subject_name}.")
                } else {
                    format!("Tugas Mandiri {idx}: Lakukan pengamatan dan susun laporan terstruktur mengenai {topic} sesuai panduan modul {subject_name}.")
                },
                question_type: "ESSAY".to_string(),
                points: (100 / (effective_num_essay.max(1) as i32)).max(10),
                image_url: None,
                choices: Vec::new(),
                interaction_config: None,
                explanation: if is_english {
                    format!("Rubric: Assessment is based on completeness, original analysis, and accurate English usage in {subject_name}.")
                } else {
                    format!("Rubrik: Penilaian berbasis kelengkapan laporan, orisinalitas analisis, dan ketepatan kaidah {subject_name}.")
                },
            });
        }
        questions.truncate(effective_num_essay);
    }

    if include_images {
        ensure_assignment_visual_guarantee(
            pool,
            &mut questions,
            subject_name,
            topic,
            grade_level,
            is_english,
        )
        .await;
    }

    Ok(GeneratedAssignmentDto {
        title: title_str,
        instructions: instructions_str,
        assignment_type: parsed.assignment_type.unwrap_or_else(|| {
            if is_homework {
                "HOMEWORK_PR".to_string()
            } else {
                "STRUCTURED_QUESTIONS".to_string()
            }
        }),
        rubric: rubric_str,
        tasks: tasks_vec,
        questions,
    })
}

async fn ensure_assignment_visual_guarantee(
    pool: &sqlx::PgPool,
    questions: &mut Vec<AssignmentQuestionDto>,
    subject_name: &str,
    topic: &str,
    grade_level: &str,
    is_english: bool,
) {
    if questions.is_empty() || questions.iter().any(|q| q.image_url.is_some()) {
        return;
    }

    let (v_text, v_choices_raw, v_correct_idx, v_expl, v_spec) =
        crate::infrastructure::visual_diagram::build_verified_visual_mcq(subject_name, topic, is_english);
    let Some(v_url) = crate::infrastructure::visual_diagram::resolve_visual_diagram_url(
        pool,
        Some(&v_spec),
        subject_name,
        grade_level,
    )
    .await
    else {
        return;
    };

    if let Some(mcq) = questions
        .iter_mut()
        .find(|q| q.question_type == "MULTIPLE_CHOICE")
    {
        mcq.question_text = v_text;
        mcq.choices = v_choices_raw
            .into_iter()
            .enumerate()
            .map(|(i, text)| CbtQuestionChoiceDto {
                choice_text: text,
                is_correct: i == v_correct_idx,
            })
            .collect();
        mcq.explanation = v_expl;
        mcq.image_url = Some(v_url);
        return;
    }

    if let Some(first_q) = questions.first_mut() {
        first_q.image_url = Some(v_url);
        if first_q.question_type == "ESSAY" {
            let prefix = if is_english {
                "Observe the visual diagram above carefully to support your analysis.\n\n"
            } else {
                "Perhatikan diagram visual di atas dengan cermat sebagai stimulus analisis Anda.\n\n"
            };
            if !first_q.question_text.contains("diagram") {
                first_q.question_text = format!("{prefix}{}", first_q.question_text);
            }
        }
    }
}

async fn ensure_quiz_visual_guarantee(
    pool: &sqlx::PgPool,
    questions: &mut Vec<CbtQuestionDto>,
    subject_name: &str,
    topic: &str,
    grade_level: &str,
    is_english: bool,
) {
    if questions.is_empty() || questions.iter().any(|q| q.image_url.is_some()) {
        return;
    }

    let (v_text, v_choices_raw, v_correct_idx, v_expl, v_spec) =
        crate::infrastructure::visual_diagram::build_verified_visual_mcq(subject_name, topic, is_english);
    let Some(v_url) = crate::infrastructure::visual_diagram::resolve_visual_diagram_url(
        pool,
        Some(&v_spec),
        subject_name,
        grade_level,
    )
    .await
    else {
        return;
    };

    let key_labels = ["A", "B", "C", "D"];
    if let Some(mcq) = questions
        .iter_mut()
        .find(|q| q.question_type.as_deref().unwrap_or("MULTIPLE_CHOICE") == "MULTIPLE_CHOICE")
    {
        mcq.question_text = v_text;
        mcq.choices = v_choices_raw
            .into_iter()
            .enumerate()
            .map(|(i, text)| CbtQuestionChoiceDto {
                choice_text: text,
                is_correct: i == v_correct_idx,
            })
            .collect();
        mcq.correct_key = key_labels[v_correct_idx % 4].to_string();
        mcq.explanation = v_expl;
        mcq.image_url = Some(v_url);
        return;
    }

    if let Some(first_q) = questions.first_mut() {
        first_q.image_url = Some(v_url);
    }
}

fn normalize_fill_blank_template_tokens(raw_tpl: &str) -> String {
    let mut result = String::with_capacity(raw_tpl.len() + 8);
    let chars: Vec<char> = raw_tpl.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut next_idx = 1usize;

    while i < len {
        // Preserve `{{n}}` as canonical placeholder
        if i + 4 < len && chars[i] == '{' && chars[i + 1] == '{' {
            if let Some(end_offset) = chars[i + 2..].windows(2).position(|w| w == ['}', '}']) {
                let inner: String = chars[i + 2..i + 2 + end_offset].iter().collect();
                if let Ok(num) = inner.trim().parse::<usize>() {
                    result.push_str(&format!("{{{{{num}}}}}"));
                    if num >= next_idx {
                        next_idx = num + 1;
                    }
                    i += 2 + end_offset + 2;
                    continue;
                }
            }
        }
        // Convert `{n}` (single brace number, e.g. from unescaped format! prompts) -> `{{n}}`
        if chars[i] == '{' {
            if let Some(end_offset) = chars[i + 1..].iter().position(|&c| c == '}') {
                let inner: String = chars[i + 1..i + 1 + end_offset].iter().collect();
                let trimmed = inner.trim();
                if let Ok(num) = trimmed.parse::<usize>() {
                    result.push_str(&format!("{{{{{num}}}}}"));
                    if num >= next_idx {
                        next_idx = num + 1;
                    }
                    i += 1 + end_offset + 1;
                    continue;
                }
                if trimmed.eq_ignore_ascii_case("blank") || trimmed.chars().all(|c| c == '_') && !trimmed.is_empty() {
                    result.push_str(&format!("{{{{{next_idx}}}}}"));
                    next_idx += 1;
                    i += 1 + end_offset + 1;
                    continue;
                }
            }
        }
        // Convert `[n]`, `[blank]`, `[___]` -> `{{k}}`
        if chars[i] == '[' {
            if let Some(end_offset) = chars[i + 1..].iter().position(|&c| c == ']') {
                let inner: String = chars[i + 1..i + 1 + end_offset].iter().collect();
                let trimmed = inner.trim();
                if let Ok(num) = trimmed.parse::<usize>() {
                    result.push_str(&format!("{{{{{num}}}}}"));
                    if num >= next_idx {
                        next_idx = num + 1;
                    }
                    i += 1 + end_offset + 1;
                    continue;
                }
                if trimmed.eq_ignore_ascii_case("blank") || (!trimmed.is_empty() && trimmed.chars().all(|c| c == '_')) {
                    result.push_str(&format!("{{{{{next_idx}}}}}"));
                    next_idx += 1;
                    i += 1 + end_offset + 1;
                    continue;
                }
            }
        }
        // Convert runs of 2+ underscores `__`, `___` -> sequential `{{k}}`
        if chars[i] == '_' && i + 1 < len && chars[i + 1] == '_' {
            while i < len && chars[i] == '_' {
                i += 1;
            }
            result.push_str(&format!("{{{{{next_idx}}}}}"));
            next_idx += 1;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

fn extract_clean_string_list(arr_opt: Option<&serde_json::Value>) -> Vec<String> {
    arr_opt
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    if let Some(s) = item.as_str() {
                        let t = s.trim();
                        if !t.is_empty() { Some(t.to_string()) } else { None }
                    } else {
                        item.get("word")
                            .or_else(|| item.get("answer"))
                            .or_else(|| item.get("text"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_fill_blank_interaction_config(cfg: &serde_json::Value, fallback_q_text: &str) -> serde_json::Value {
    let raw_tpl = cfg
        .get("template")
        .or_else(|| cfg.get("sentence"))
        .or_else(|| cfg.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let mut template = normalize_fill_blank_template_tokens(raw_tpl);
    if !template.contains("{{") {
        let fallback_tpl = normalize_fill_blank_template_tokens(fallback_q_text.trim());
        if fallback_tpl.contains("{{") {
            template = fallback_tpl;
        }
    }

    let mut blanks = extract_clean_string_list(
        cfg.get("blanks")
            .or_else(|| cfg.get("answers"))
            .or_else(|| cfg.get("correct_answers"))
            .or_else(|| cfg.get("keys")),
    );
    if blanks.is_empty() {
        blanks = extract_clean_string_list(cfg.get("word_bank"));
    }
    let distractors = extract_clean_string_list(cfg.get("distractors").or_else(|| cfg.get("wrong_options")));

    serde_json::json!({
        "template": template,
        "blanks": blanks,
        "distractors": distractors
    })
}

fn is_valid_game_interaction_config(question_type: &str, cfg: &serde_json::Value) -> bool {
    match question_type {
        "MATCHING" => cfg
            .get("pairs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|p| {
                        let left = p.get("left").and_then(|v| v.as_str()).unwrap_or("").trim();
                        let right = p.get("right").and_then(|v| v.as_str()).unwrap_or("").trim();
                        !left.is_empty() && !right.is_empty()
                    })
                    .count()
                    >= 2
            })
            .unwrap_or(false),
        "ORDERING" => cfg
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|it| !it.get("text").and_then(|v| v.as_str()).unwrap_or("").trim().is_empty())
                    .count()
                    >= 2
            })
            .unwrap_or(false),
        "FILL_BLANK" => {
            let tpl = cfg.get("template").and_then(|v| v.as_str()).unwrap_or("").trim();
            let has_slot = tpl.contains("___") || (tpl.contains("{{") && tpl.contains("}}"));
            let has_blanks = cfg
                .get("blanks")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .any(|b| !b.as_str().unwrap_or("").trim().is_empty())
                })
                .unwrap_or(false);
            has_slot && has_blanks
        }
        "CROSSWORD" => {
            let valid_clues = cfg
                .get("clues")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len() >= 2)
                .unwrap_or(false);
            let valid_words = cfg
                .get("words")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter(|w| {
                            let ans_len = w
                                .get("answer")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .chars()
                                .filter(|c| c.is_ascii_alphabetic())
                                .count();
                            let clue = w.get("clue").and_then(|v| v.as_str()).unwrap_or("").trim();
                            (2..=15).contains(&ans_len) && !clue.is_empty()
                        })
                        .count()
                        >= 2
                })
                .unwrap_or(false);
            valid_clues || valid_words
        }
        _ => false,
    }
}

fn build_fallback_matching_question(topic: &str, subject: &str, ts: i64) -> AssignmentQuestionDto {
    if is_english_subject(subject) {
        return AssignmentQuestionDto {
            id: format!("asg-match-fb-{ts}"),
            question_text: format!("Match each key concept of '{topic}' ({subject}) with its accurate explanation:"),
            question_type: "MATCHING".to_string(),
            points: 20,
            image_url: None,
            choices: Vec::new(),
            interaction_config: Some(serde_json::json!({
                "pairs": [
                    { "id": "p1", "left": format!("Core Concept of {topic}"), "right": "Main foundation and key vocabulary of this English topic" },
                    { "id": "p2", "left": "Contextual Meaning", "right": "Understanding words and phrases through real situations" },
                    { "id": "p3", "left": "Sentence Structure", "right": "Organizing expressions with accurate English grammar" },
                    { "id": "p4", "left": "Real-Life Practice", "right": "Using the expressions in everyday speaking and writing" }
                ]
            })),
            explanation: format!("Each pair represents a core learning stage for mastering {topic} in English."),
        };
    }

    AssignmentQuestionDto {
        id: format!("asg-match-fb-{ts}"),
        question_text: format!("Jodohkan konsep {topic} ({subject}) berikut dengan penjelasan yang paling sesuai:"),
        question_type: "MATCHING".to_string(),
        points: 20,
        image_url: None,
        choices: Vec::new(),
        interaction_config: Some(serde_json::json!({
            "pairs": [
                { "id": "p1", "left": format!("Konsep Dasar {topic}"), "right": "Fondasi teori utama pembelajaran topik ini" },
                { "id": "p2", "left": "Identifikasi Masalah", "right": "Mengamati persoalan nyata sesuai topik" },
                { "id": "p3", "left": "Analisis Kritis", "right": "Menghubungkan sebab akibat secara logis" },
                { "id": "p4", "left": "Penerapan Praktis", "right": "Implementasi langsung dalam keseharian" }
            ]
        })),
        explanation: format!("Setiap pasangan mencerminkan tahapan penguasaan konsep {topic}."),
    }
}

fn build_fallback_ordering_question(topic: &str, subject: &str, ts: i64) -> AssignmentQuestionDto {
    if is_english_subject(subject) {
        return AssignmentQuestionDto {
            id: format!("asg-order-fb-{ts}"),
            question_text: format!("Arrange the steps to learn and practice '{topic}' ({subject}) from first to last:"),
            question_type: "ORDERING".to_string(),
            points: 20,
            image_url: None,
            choices: Vec::new(),
            interaction_config: Some(serde_json::json!({
                "items": [
                    { "id": "o1", "text": format!("Recognize the key vocabulary and expressions for {topic}") },
                    { "id": "o2", "text": "Read sample sentences and short dialogues carefully" },
                    { "id": "o3", "text": "Complete guided grammar and usage exercises" },
                    { "id": "o4", "text": "Write or speak your own sentences fluently" }
                ]
            })),
            explanation: "Effective English learning moves from vocabulary recognition and reading examples to guided practice and communication.".to_string(),
        };
    }

    AssignmentQuestionDto {
        id: format!("asg-order-fb-{ts}"),
        question_text: format!("Urutkan tahapan mempelajari dan menerapkan '{topic}' ({subject}) dari awal hingga akhir:"),
        question_type: "ORDERING".to_string(),
        points: 20,
        image_url: None,
        choices: Vec::new(),
        interaction_config: Some(serde_json::json!({
            "items": [
                { "id": "o1", "text": format!("Memahami definisi inti materi {topic}") },
                { "id": "o2", "text": "Mengidentifikasi contoh dan komponen penting" },
                { "id": "o3", "text": "Menerapkan konsep untuk memecahkan latihan soal" },
                { "id": "o4", "text": "Menarik kesimpulan dan refleksi pembelajaran" }
            ]
        })),
        explanation: "Urutan belajar dimulai dari pemahaman konsep dasar hingga refleksi.".to_string(),
    }
}

fn build_fallback_fill_blank_question(topic: &str, subject: &str, ts: i64) -> AssignmentQuestionDto {
    if is_english_subject(subject) {
        return AssignmentQuestionDto {
            id: format!("asg-fill-fb-{ts}"),
            question_text: format!("Complete the English learning summary on '{topic}' ({subject}) using the correct words:"),
            question_type: "FILL_BLANK".to_string(),
            points: 20,
            image_url: None,
            choices: Vec::new(),
            interaction_config: Some(serde_json::json!({
                "template": format!("In {subject}, mastering {topic} begins with learning key {{1}} and applying it in real {{2}}."),
                "blanks": ["vocabulary", "conversation"],
                "distractors": ["silence", "confusion"]
            })),
            explanation: "Learning key vocabulary and practicing it in real conversation builds lasting fluency.".to_string(),
        };
    }

    AssignmentQuestionDto {
        id: format!("asg-fill-fb-{ts}"),
        question_text: format!("Lengkapi ringkasan konsep '{topic}' ({subject}) dengan pilihan kata yang tepat:"),
        question_type: "FILL_BLANK".to_string(),
        points: 20,
        image_url: None,
        choices: Vec::new(),
        interaction_config: Some(serde_json::json!({
            "template": format!("Dalam mata pelajaran {subject}, penguasaan topik {topic} diawali dengan memahami {{1}} lalu diterapkan lewat {{2}} nyata."),
            "blanks": ["konsep dasar", "latihan"],
            "distractors": ["hafalan pasif", "dugaan acak"]
        })),
        explanation: "Pemahaman konsep dasar dan latihan terarah adalah kunci penguasaan materi.".to_string(),
    }
}

fn build_fallback_crossword_question(topic: &str, subject: &str, ts: i64) -> AssignmentQuestionDto {
    let raw_cfg = if is_english_subject(subject) {
        serde_json::json!({
            "words": [
                { "answer": "CONCEPT", "clue": format!("Core idea learned in {topic}") },
                { "answer": "CONTEXT", "clue": "Situation in which an idea or phrase appears" },
                { "answer": "PRACTICE", "clue": "Repeated exercise to improve mastery" },
                { "answer": "EXAMPLE", "clue": "Concrete case illustrating a rule" }
            ]
        })
    } else {
        serde_json::json!({
            "words": [
                { "answer": "KONSEP", "clue": format!("Ide atau gagasan dasar pada materi {topic}") },
                { "answer": "PROSES", "clue": "Rangkaian tahapan atau perubahan peristiwa" },
                { "answer": "PRAKTIK", "clue": "Penerapan nyata konsep yang dipelajari" },
                { "answer": "ANALISIS", "clue": "Kegiatan menguraikan masalah untuk menemukan solusi" }
            ]
        })
    };
    let compiled = crate::presentation::learning::assignments::interactive_game::compile_crossword_interaction_config(&raw_cfg);
    AssignmentQuestionDto {
        id: format!("asg-cw-fb-{ts}"),
        question_text: if is_english_subject(subject) {
            format!("Complete the crossword puzzle about '{topic}' ({subject}) using the clues below:")
        } else {
            format!("Isilah Teka-Teki Silang (TTS) seputar topik '{topic}' ({subject}) berikut sesuai petunjuk:")
        },
        question_type: "CROSSWORD".to_string(),
        points: 20,
        image_url: None,
        choices: Vec::new(),
        interaction_config: Some(compiled),
        explanation: if is_english_subject(subject) {
            "Reviewing key terms strengthens vocabulary and comprehension.".to_string()
        } else {
            "Penguasaan kosakata dan istilah kunci memperkuat pemahaman konsep.".to_string()
        },
    }
}

pub fn generate_pedagogical_question_templates(
    subject: &str,
    topic: &str,
    grade_level: &str,
    difficulty: &str,
    count: usize,
) -> Vec<(String, Vec<String>, usize, String)> {
    let subject_name = subject;
    let lower = subject.trim().to_lowercase();
    let is_english = lower.contains("inggris") || lower.contains("english");
    let is_math = lower.contains("matematika") || lower.contains("math") || lower.contains("aljabar") || lower.contains("geometri") || lower.contains("kalkulus");
    let is_science = lower.contains("ipa") || lower.contains("sains") || lower.contains("fisika") || lower.contains("biologi") || lower.contains("kimia");
    let is_social = lower.contains("ips") || lower.contains("sejarah") || lower.contains("geografi") || lower.contains("sosiologi") || lower.contains("ekonomi");

    let base_templates: Vec<(String, Vec<String>, usize, String)> = if is_english {
        vec![
            (
                format!("What is the primary function of learning '{topic}' in {grade_level} English language studies?"),
                vec![
                    "To enhance contextual communicative competence and subject-specific vocabulary".to_string(),
                    "To memorize isolated grammatical rules without practical communication".to_string(),
                    "To replace modern sentence structures with archaic dialects".to_string(),
                    "To avoid formal writing conventions in academic discourse".to_string(),
                ],
                0,
                format!("Studying {topic} aims to foster authentic communicative proficiency and analytical expression in English."),
            ),
            (
                format!("Which of the following best exemplifies an accurate application of '{topic}'?"),
                vec![
                    "Ignoring cohesive markers and syntactic word order in discourse".to_string(),
                    "Employing appropriate terminology with consistent grammatical agreement".to_string(),
                    "Translating sentences verbatim without considering pragmatic meaning".to_string(),
                    "Using informal expressions exclusively in formal documentation".to_string(),
                ],
                1,
                format!("Accurate application of {topic} requires linguistic coherence, grammatical precision, and contextual awareness."),
            ),
            (
                format!("When analyzing a reading text concerning '{topic}', which analytical strategy is most recommended?"),
                vec![
                    "Relying strictly on personal assumptions without textual citations".to_string(),
                    "Skipping introductory and summary paragraphs entirely".to_string(),
                    "Identifying main ideas, supporting arguments, and contextual inferences".to_string(),
                    "Disregarding unfamiliar key terminology".to_string(),
                ],
                2,
                format!("Reading comprehension under Kurikulum Merdeka emphasizes evidence-based inference and critical text analysis."),
            ),
            (
                format!("In communicative tasks, how should students evaluate their presentation on '{topic}'?"),
                vec![
                    "By speaking rapidly regardless of audience comprehension".to_string(),
                    "By omitting logical transitions between concepts".to_string(),
                    "By avoiding interactive engagement with peers".to_string(),
                    "By assessing clarity of delivery, audience engagement, and logical organization".to_string(),
                ],
                3,
                format!("Effective communication is measured by structural clarity, coherence, and successful message transmission."),
            ),
            (
                format!("How does mastering '{topic}' contribute to interdisciplinary learning for {grade_level} students?"),
                vec![
                    "It equips students with communicative tools to analyze and synthesize global academic resources".to_string(),
                    "It limits analytical thinking strictly to localized vocabulary".to_string(),
                    "It prevents integration with scientific and mathematical concepts".to_string(),
                    "It replaces analytical discussion with repetitive mechanical drills".to_string(),
                ],
                0,
                format!("English serves as an academic bridge language for multidimensional knowledge acquisition."),
            ),
        ]
    } else if is_math {
        vec![
            (
                format!("Dalam mempelajari konsep '{topic}' ({grade_level}), manakah pernyataan matematis berikut yang paling tepat menggambarkan relasi antar-variabelnya?"),
                vec![
                    "Relasi proporsional terstruktur yang menghubungkan input dan output secara konsisten".to_string(),
                    "Hubungan acak tanpa pola keteraturan matematis".to_string(),
                    "Persamaan yang mengabaikan urutan hierarki operasi hitung baku".to_string(),
                    "Nilai konstan mutlak yang tidak dipengaruhi parameter kondisi awal".to_string(),
                ],
                0,
                format!("Konsep {topic} dalam matematika bertumpu pada relasi logis, pola fungsi, dan kaidah operasi hitung yang teruji."),
            ),
            (
                format!("Pada sebuah perhitungan kontekstual terkait '{topic}', jika suatu besaran awal bernilai 40 meningkat sebesar 25%, berapakah nilai akhirnya?"),
                vec![
                    "45".to_string(),
                    "50".to_string(),
                    "55".to_string(),
                    "60".to_string(),
                ],
                1,
                format!("Kenaikan 25% dari 40 adalah 0,25 x 40 = 10. Nilai akhir = 40 + 10 = 50."),
            ),
            (
                format!("Diberikan permasalahan kontekstual materi '{topic}': suatu persamaan linear memenuhi $2x - 6 = 18$. Berapakah nilai dari $x^2 - 14$?"),
                vec![
                    "116".to_string(),
                    "125".to_string(),
                    "130".to_string(),
                    "144".to_string(),
                ],
                2,
                format!("Dari persamaan $2x = 24$, diperoleh $x = 12$. Maka $x^2 - 14 = 144 - 14 = 130$."),
            ),
            (
                format!("Langkah awal yang paling sistematis dalam menyelesaikan soal cerita berbasis pemecahan masalah materi '{topic}' adalah..."),
                vec![
                    "Langsung menebak hasil akhir tanpa merumuskan variabel permasalahan".to_string(),
                    "Mengabaikan satuan besaran dan konteks cerita yang diberikan".to_string(),
                    "Mengalikan seluruh angka yang muncul tanpa membuat model persamaan".to_string(),
                    "Mengidentifikasi data yang diketahui, besaran yang ditanyakan, lalu menyusun model matematika".to_string(),
                ],
                3,
                format!("Pemecahan masalah matematis analitis diawali dengan dekomposisi parameter dan formulasi model matematika yang valid."),
            ),
            (
                format!("Sebuah kumpulan data pengamatan tentang '{topic}' memiliki rata-rata 80 dari 4 kali percobaan awal. Jika pada percobaan kelima diperoleh nilai 95, berapa rata-rata barunya?"),
                vec![
                    "83".to_string(),
                    "85".to_string(),
                    "87".to_string(),
                    "81".to_string(),
                ],
                0,
                format!("Total nilai awal = 4 x 80 = 320. Total nilai baru = 320 + 95 = 415. Rata-rata baru = 415 / 5 = 83."),
            ),
        ]
    } else if is_science {
        vec![
            (
                format!("Berdasarkan metode ilmiah, variabel bebas yang paling relevan dalam menyelidiki fenomena '{topic}' adalah..."),
                vec![
                    "Faktor perlakuan yang sengaja diubah oleh peneliti untuk mengamati respons sistem".to_string(),
                    "Parameter pengamatan yang dijaga tetap konstan sepanjang percobaan".to_string(),
                    "Hasil akhir yang dicatat setelah siklus reaksi berakhir".to_string(),
                    "Wadah eksperimen yang digunakan di laboratorium".to_string(),
                ],
                0,
                format!("Variabel bebas adalah perlakuan yang sengaja dimanipulasi untuk mengukur dampaknya terhadap variabel terikat."),
            ),
            (
                format!("Mekanisme keterkaitan sebab-akibat yang mendasari prinsip kerja materi '{topic}' ditunjukkan oleh peristiwa..."),
                vec![
                    "Perubahan energi spontan yang tidak mematuhi hukum kekekalan".to_string(),
                    "Interaksi sistemik di mana perubahan input memicu respons adaptif pada kesetimbangan sistem".to_string(),
                    "Terjadinya reaksi tanpa adanya gaya pemicu atau gradien konsentrasi".to_string(),
                    "Penyusutan massa zat tanpa konversi radiasi".to_string(),
                ],
                1,
                format!("Fenomena sains bekerja berdasarkan interaksi dinamis dan hukum kekekalan materi serta energi."),
            ),
            (
                format!("Bagaimana dampak penerapan teknologi ramah lingkungan berbasis prinsip '{topic}' terhadap keberlanjutan ekosistem?"),
                vec![
                    "Meningkatkan pelepasan emisi karbon dan polusi kimiawi".to_string(),
                    "Mengabaikan efisiensi daur ulang materi dan energi".to_string(),
                    "Mengoptimalkan efisiensi energi serta meminimalkan residu limbah pencemar lingkungan".to_string(),
                    "Menghentikan seluruh aktivitas riset laboratorium secara permanen".to_string(),
                ],
                2,
                format!("Teknologi sains Kurikulum Merdeka berorientasi pada keberlanjutan ekologis dan efisiensi sumber daya alam."),
            ),
            (
                format!("Dalam melakukan pengukuran eksperimental fenomena '{topic}', prosedur validasi data yang paling tepat adalah..."),
                vec![
                    "Mengandalkan taksiran visual sepintas tanpa kalibrasi alat".to_string(),
                    "Mencatat satu kali pembacaan skala tanpa uji pengulangan".to_string(),
                    "Memanipulasi angka agar selalu cocok dengan ekspektasi awal".to_string(),
                    "Menggunakan instrumen terkalibrasi dan melakukan pengukuran berulang untuk memperoleh data rerata yang presisi".to_string(),
                ],
                3,
                format!("Presisi dan reliabilitas saintifik diperoleh melalui kalibrasi instrumen dan replikasi pengukuran empiris."),
            ),
            (
                format!("Korelasi antara struktur dan fungsi pada komponen kajian '{topic}' menggambarkan prinsip bahwa..."),
                vec![
                    "Bentuk struktural materi atau organ dirancang spesifik untuk mengoptimalkan kinerja fungsionalnya".to_string(),
                    "Struktur material sama sekali tidak berhubungan dengan sifat fisik atau biologisnya".to_string(),
                    "Fungsi dapat berjalan optimal tanpa memerlukan keteraturan struktur".to_string(),
                    "Semua komponen penyusun memiliki sifat mekanis yang identik tanpa diferensiasi".to_string(),
                ],
                0,
                format!("Prinsip universal sains menyatakan bahwa karakteristik struktural menentukan kapabilitas fungsional."),
            ),
        ]
    } else if is_social {
        vec![
            (
                format!("Ditinjau dari perspektif interaksi keruangan, faktor pendorong utama terjadinya fenomena '{topic}' adalah..."),
                vec![
                    "Perbedaan potensi sumber daya dan kebutuhan saling melengkapi (komplementaritas) antarwilayah".to_string(),
                    "Keterisolasian mutlak yang menghambat komunikasi antar-kelompok masyarakat".to_string(),
                    "Keseragaman bentang alam yang menghilangkan motif pertukaran".to_string(),
                    "Penolakan terhadap segala bentuk inovasi dan pembangunan".to_string(),
                ],
                0,
                format!("Dinamika geografis dan sosial digerakkan oleh komplementaritas spasial akibat variasi potensi wilayah."),
            ),
            (
                format!("Bagaimana pengaruh dinamika historis dan kearifan lokal terkait '{topic}' terhadap kehidupan sosial masyarakat saat ini?"),
                vec![
                    "Menghapuskan seluruh identitas kultural generasi terdahulu".to_string(),
                    "Mewariskan nilai gotong royong, identitas kebangsaan, dan ketahanan sosial yang adaptif".to_string(),
                    "Menghalangi keterbukaan masyarakat terhadap ilmu pengetahuan modern".to_string(),
                    "Memicu fragmentasi sosial yang melemahkan persatuan".to_string(),
                ],
                1,
                format!("Nilai historis dan sosial memperkuat kohesi sosial dan Profil Pelajar Pancasila dalam bermasyarakat."),
            ),
            (
                format!("Dalam kerangka pembangunan berkelanjutan, pengelolaan potensi '{topic}' secara bijak harus mengedepankan..."),
                vec![
                    "Pemanfaatan sumber daya alam tanpa batas demi laba sesaat".to_string(),
                    "Pemusatan fasilitas publik hanya di wilayah perkotaan".to_string(),
                    "Keseimbangan proporsional antara kemajuan ekonomi, keadilan sosial, dan pelestarian lingkungan hidup".to_string(),
                    "Pengabaian aspirasi masyarakat lokal dalam penentuan kebijakan".to_string(),
                ],
                2,
                format!("Pembangunan berkelanjutan mensinergikan pilar ekonomi, keadilan sosial, dan kelestarian ekologi."),
            ),
            (
                format!("Peran proaktif yang dapat dijalankan oleh generasi muda dalam mengoptimalkan potensi '{topic}' di era globalisasi adalah..."),
                vec![
                    "Menjadi konsumen pasif produk luar tanpa daya saing".to_string(),
                    "Menolak perkembangan teknologi informasi terkini".to_string(),
                    "Mengabaikan kelestarian warisan budaya daerah sendiri".to_string(),
                    "Mengembangkan inovasi kreatif, literasi digital kritis, dan solusi nyata bagi kemajuan komunitas".to_string(),
                ],
                3,
                format!("Generasi muda Kurikulum Merdeka berperan sebagai agen perubahan yang kreatif, kritis, dan berwawasan kebangsaan."),
            ),
            (
                format!("Keterkaitan antara bentang alam fisik dan mata pencaharian penduduk pada materi '{topic}' membuktikan bahwa..."),
                vec![
                    "Kondisi geografis menyediakan peluang adaptasi yang membentuk pola sosio-ekonomi masyarakat setempat".to_string(),
                    "Aktivitas ekonomi manusia sama sekali tidak dipengaruhi oleh iklim dan topografi".to_string(),
                    "Manusia tidak memiliki kapasitas inovasi dalam menyesuaikan diri dengan lingkungan".to_string(),
                    "Pola permukiman selalu seragam di segala medan topografi".to_string(),
                ],
                0,
                format!("Pola sosio-ekonomi masyarakat berkembang sebagai respons adaptif terhadap karakteristik bentang alam geografis."),
            ),
        ]
    } else {
        vec![
            (
                format!("Konsep fundamental yang menjadi inti bahasan materi '{topic}' pada mata pelajaran {subject_name} ({grade_level}) adalah..."),
                vec![
                    "Landasan konseptual terstruktur yang menghubungkan pemahaman teori dengan pemecahan masalah kontekstual".to_string(),
                    "Kumpulan hafalan definisi tanpa kaitan dengan penerapan kehidupan nyata".to_string(),
                    "Kaidah teoritis semata yang tidak dapat diuji atau diterapkan secara praktis".to_string(),
                    "Asumsi subjektif yang belum terverifikasi secara metodologis".to_string(),
                ],
                0,
                format!("Pembelajaran bermakna Kurikulum Merdeka menitikberatkan pada pemahaman konseptual dan aplikasinya di dunia nyata."),
            ),
            (
                format!("Ciri utama yang menandai penguasaan kompetensi tingkat {difficulty} pada materi '{topic}' adalah kemampuan siswa untuk..."),
                vec![
                    "Menyalin teks materi secara berulang tanpa menganalisis maknanya".to_string(),
                    "Melakukan penalaran logis, mengevaluasi bukti, dan merumuskan kesimpulan analitis secara mandiri".to_string(),
                    "Menghindari pertanyaan reflektif yang memerlukan pertimbangan kritis".to_string(),
                    "Menjawab secara terburu-buru tanpa memperhatikan kaidah ilmu terkait".to_string(),
                ],
                1,
                format!("Karakteristik asesmen Kurikulum Merdeka adalah penguatan daya nalar kritis dan pemecahan masalah mandiri."),
            ),
            (
                format!("Dalam menghadapi permasalahan kontekstual mengenai '{topic}', tindakan awal yang paling tepat dilakukan adalah..."),
                vec![
                    "Menarik kesimpulan tanpa memeriksa data dan fakta pendukung".to_string(),
                    "Mengabaikan pandangan atau masukan yang berbeda dari pihak lain".to_string(),
                    "Menelaah situasi secara objektif, memetakan akar masalah, dan merancang alternatif pemecahan yang terukur".to_string(),
                    "Menyerahkan penyelesaian sepenuhnya kepada orang lain tanpa keterlibatan aktif".to_string(),
                ],
                2,
                format!("Pemecahan masalah sistematis menuntut observasi cermat, identifikasi akar persoalan, dan formulasi solusi."),
            ),
            (
                format!("Penerapan etika, integritas, dan kolaborasi positif saat mempelajari materi '{topic}' diwujudkan melalui..."),
                vec![
                    "Memanfaatkan pengetahuan hanya untuk keuntungan pribadi dengan mengabaikan norma bersama".to_string(),
                    "Menolak bekerja sama dengan rekan kelompok yang berbeda latar belakang".to_string(),
                    "Menyampaikan argumen secara sepihak tanpa menghargai etika komunikasi".to_string(),
                    "Menghargai keragaman pendapat, bertanggung jawab atas hasil kerja, dan mengutamakan kemaslahatan bersama".to_string(),
                ],
                3,
                format!("Profil Pelajar Pancasila menekankan integritas, gotong royong, dan akhlak mulia dalam penerapan ilmu."),
            ),
            (
                format!("Langkah reflektif yang paling konstruktif untuk memperdalam pemahaman materi '{topic}' secara berkelanjutan adalah..."),
                vec![
                    "Mengevaluasi proses belajar mandiri, mengidentifikasi ruang pengembangan, dan menindaklanjuti umpan balik".to_string(),
                    "Hanya mempelajari materi saat menjelang waktu evaluasi berkala".to_string(),
                    "Menghindari topik materi yang dirasa membutuhkan konsentrasi lebih".to_string(),
                    "Mengabaikan catatan evaluasi dari guru pengampu".to_string(),
                ],
                0,
                format!("Metakognisi dan evaluasi diri merupakan pilar utama pembelajaran sepanjang hayat."),
            ),
        ]
    };

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let base = &base_templates[i % base_templates.len()];
        let q_num = i + 1;
        let mut q_text = base.0.clone();
        if i >= base_templates.len() {
            q_text = if is_english {
                format!("(Item {q_num}) {q_text}")
            } else {
                format!("(Butir {q_num}) {q_text}")
            };
        }
        result.push((q_text, base.1.clone(), base.2, base.3.clone()));
    }
    result
}

pub fn fallback_generate_assignment(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    assignment_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    _include_images: bool,
) -> GeneratedAssignmentDto {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let is_english = is_english_subject(subject_name);
    let fmt_upper = assignment_format.unwrap_or("STRUCTURED_QUESTIONS").to_uppercase();
    let is_homework = fmt_upper == "HOMEWORK_PR";
    let is_pure_game = fmt_upper == "INTERACTIVE_GAMES";
    let is_game_mixed = fmt_upper.contains("GAME") && !is_pure_game;

    let safe_image: Option<String> = None;

    let mcq_templates = if is_homework || is_pure_game || num_mcq == 0 {
        Vec::new()
    } else {
        generate_pedagogical_question_templates(subject_name, topic, grade_level, difficulty, num_mcq)
    };
    let mut questions = Vec::new();

    if !is_homework && !is_pure_game && num_mcq > 0 {
        for (idx, (q_text, choices_text, correct_idx, expl)) in mcq_templates.into_iter().enumerate() {
            let choices = choices_text
                .into_iter()
                .enumerate()
                .map(|(c_idx, text)| CbtQuestionChoiceDto {
                    choice_text: text,
                    is_correct: c_idx == correct_idx,
                })
                .collect();

            questions.push(AssignmentQuestionDto {
                id: format!("asg-mcq-{}-{now_ts}", idx + 1),
                question_text: q_text,
                question_type: "MULTIPLE_CHOICE".to_string(),
                points: 10,
                image_url: safe_image.clone(),
                choices,
                interaction_config: None,
                explanation: expl,
            });
        }
    }

    if is_pure_game || is_game_mixed {
        questions.push(build_fallback_matching_question(topic, subject_name, now_ts));
        questions.push(build_fallback_ordering_question(topic, subject_name, now_ts));
        questions.push(build_fallback_fill_blank_question(topic, subject_name, now_ts));
        questions.push(build_fallback_crossword_question(topic, subject_name, now_ts));
    }

    let essay_count = if is_homework {
        num_essay.max(1)
    } else if is_pure_game || is_game_mixed {
        0
    } else {
        num_essay
    };
    let essay_pts = (100 / essay_count.max(1) as i32).max(15);

    let essay_prompts = if is_english {
        [
            format!("Write a clear paragraph in English explaining what you learned about '{topic}' ({subject_name}, {grade_level}), and include at least two real-life examples!"),
            format!("Create a short English dialogue (4-6 lines) featuring key vocabulary and expressions from '{topic}', and briefly explain its communicative purpose."),
            format!("Analyze how mastering vocabulary and expressions related to '{topic}' helps students communicate effectively in everyday English conversations."),
            format!("Write a brief self-reflection in English summarizing the most important expressions you learned in '{topic}' and how you will practice them."),
        ]
    } else {
        [
            format!("Jelaskan pemahaman komprehensif Anda mengenai konsep '{topic}' pada mata pelajaran {subject_name} ({grade_level}). Tuliskan minimal dua contoh penerapan nyata dalam kehidupan sehari-hari!"),
            format!("Lakukan analisis terhadap sebuah permasalahan kontekstual yang berkaitan dengan materi '{topic}'. Uraikan langkah-langkah pemecahan masalah secara terstruktur dan berikan kesimpulan logis!"),
            format!("Susunlah sebuah telaah kritis mengenai tantangan yang dihadapi dalam penguasaan materi '{topic}', serta usulkan ide inovatif untuk mengatasinya!"),
            format!("Buatlah refleksi pembelajaran mendalam mengenai nilai-nilai baru yang Anda peroleh setelah mempelajari '{topic}', dan bagaimana materi ini membentuk cara berpikir kritis Anda!"),
        ]
    };

    for idx in 0..essay_count {
        let prompt_text = essay_prompts[idx % essay_prompts.len()].clone();
        let q_num = questions.len() + 1;
        questions.push(AssignmentQuestionDto {
            id: format!("asg-essay-{}-{now_ts}", idx + 1),
            question_text: if essay_count > essay_prompts.len() {
                if is_english {
                    format!("(Essay {q_num}) {prompt_text}")
                } else {
                    format!("(Uraian {q_num}) {prompt_text}")
                }
            } else {
                prompt_text
            },
            question_type: "ESSAY".to_string(),
            points: essay_pts,
            image_url: None,
            choices: Vec::new(),
            interaction_config: None,
            explanation: if is_english {
                "Scoring Rubric: Full score is awarded for clear English grammar, accurate vocabulary usage, logical ideas, and relevant examples (0 - 100 scale).".to_string()
            } else {
                "Rubrik Penskoran: Skor maksimal diberikan bila siswa menjabarkan konsep secara tepat, argumentasi logis runtut, disertai contoh kontekstual yang relevan (Skala 0 - 100).".to_string()
            },
        });
    }

    if is_english {
        let tasks = vec![
            format!("Task 1: Review the key English vocabulary and sentence patterns for '{topic}' carefully."),
            format!("Task 2: Complete each question and activity below clearly in natural English."),
            format!("Task 3: Proofread your grammar, spelling, and punctuation before submitting."),
        ];

        return GeneratedAssignmentDto {
            title: format!("English Assignment: {topic}"),
            instructions: format!("Assignment Instructions ({grade_level} - {subject_name}):\n1. Read the prompts and key expressions about '{topic}' carefully.\n2. Answer all items clearly in natural English with accurate grammar and vocabulary.\n3. Difficulty level: {difficulty}. Double-check your answers before submitting."),
            assignment_type: if is_homework { "HOMEWORK_PR".to_string() } else { "STRUCTURED_QUESTIONS".to_string() },
            rubric: "Grading Rubric (0 - 100 Scale):\n- 85 - 100: Accurate English vocabulary, clear grammar, well-developed communicative response.\n- 70 - 84: Clear meaning with minor grammatical or spelling errors.\n- 55 - 69: Basic ideas conveyed; needs additional vocabulary and sentence practice.\n- < 55: Needs guided review of core English concepts.".to_string(),
            tasks,
            questions,
        };
    }

    let tasks = vec![
        format!("Tugas 1: Pelajari rangkuman materi pokok '{topic}' dengan seksama."),
        format!("Tugas 2: Kerjakan butir soal terstruktur sesuai instruksi dan rubrik yang ditetapkan."),
        format!("Tugas 3: Lakukan evaluasi mandiri terhadap jawaban Anda sebelum mengumpulkan tugas."),
    ];

    GeneratedAssignmentDto {
        title: format!("Tugas Siswa: {topic}"),
        instructions: format!("Petunjuk Pengerjaan ({grade_level} - {subject_name}):\n1. Bacalah materi tentang '{topic}' dengan teliti.\n2. Jawab setiap butir tugas dan soal dengan sistematis, teliti, dan mengedepankan nalar kritis.\n3. Tingkat kesulitan: {difficulty}. Periksa kembali pekerjaan sebelum diserahkan."),
        assignment_type: if is_homework { "HOMEWORK_PR".to_string() } else { "STRUCTURED_QUESTIONS".to_string() },
        rubric: "Rubrik Penilaian Kurikulum Merdeka (Skor 0 - 100):\n- 85 - 100: Pemahaman konsep akurat, analisis mendalam, argumen runtut.\n- 70 - 84: Konsep tepat, penjelasan jelas, sedikit kekurangan teknis.\n- 55 - 69: Konsep dasar dipahami, membutuhkan penguatan analisis.\n- < 55: Membutuhkan pendampingan remedial dan penguatan konsep dasar.".to_string(),
        tasks,
        questions,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. CBT QUIZ / EXAM GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CbtQuestionDto {
    pub id: String,
    pub question_text: String,
    #[serde(default)]
    pub question_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub choices: Vec<CbtQuestionChoiceDto>,
    pub correct_key: String,
    pub explanation: String,
    pub bloom_level: String,
    pub points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedCbtQuizDto {
    pub title: String,
    pub description: String,
    pub questions: Vec<CbtQuestionDto>,
}

pub async fn generate_cbt_quiz(
    pool: &sqlx::PgPool,
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    quiz_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    include_images: bool,
) -> Result<GeneratedCbtQuizDto, ApplicationError> {
    let is_english = is_english_subject(subject_name);
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let visual_instructions = build_visual_diagram_prompt_instructions(is_english, include_images);
    let now_ts = chrono::Utc::now().timestamp_millis();
    let is_mixed = num_essay > 0
        || quiz_format
            .map(|f| f.to_uppercase() == "MIXED" || f.to_uppercase().contains("CAMPURAN"))
            .unwrap_or(false);
    let total_questions = if is_mixed {
        num_mcq + num_essay
    } else {
        num_mcq
    };

    let composition_instruction = if is_english {
        if is_mixed {
            format!(
                r#"CBT EXAM COMPOSITION RULES (MIXED MULTIPLE CHOICE & ESSAY — ALL IN ENGLISH):
- Multiple Choice Count: EXACTLY {num_mcq} items ('question_type': 'MULTIPLE_CHOICE').
  Each MCQ item MUST have 4 English options A, B, C, D, correct_key ('A', 'B', 'C', or 'D'), and an English explanation.
- Essay Count: EXACTLY {num_essay} items ('question_type': 'ESSAY').
  Each Essay item MUST include an English scoring rubric in 'explanation' and empty choices.
- Total length of 'questions' MUST be EXACTLY {total_questions} items ({num_mcq} MCQ + {num_essay} Essay)."#
            )
        } else {
            format!(
                r#"CBT EXAM COMPOSITION RULES (100% MULTIPLE CHOICE — ALL IN ENGLISH):
- Multiple Choice Count: EXACTLY {total_questions} items ('question_type': 'MULTIPLE_CHOICE').
  Each question MUST have 4 English options A, B, C, D, correct_key ('A', 'B', 'C', or 'D'), and an English explanation.
- Total length of 'questions' MUST be EXACTLY {total_questions} items."#
            )
        }
    } else if is_mixed {
        format!(
            r#"ATURAN KOMPOSISI SOAL CBT (CAMPURAN PG & ESSAY):
- Jumlah Soal Pilihan Ganda (PG): TEPAT {num_mcq} butir ('question_type': 'MULTIPLE_CHOICE').
  Setiap butir PG WAJIB memiliki 4 pilihan A, B, C, D dengan kunci jawaban correct_key ('A', 'B', 'C', atau 'D') dan penjelasan.
- Jumlah Soal Uraian / Essay: TEPAT {num_essay} butir ('question_type': 'ESSAY').
  Setiap butir Essay WAJIB memiliki rubrik pedoman penskoran di 'explanation', dan bagian 'options'/'choices' dikosongkan.
- Total butir dalam array 'questions' WAJIB TEPAT {total_questions} butir ({num_mcq} PG dan {num_essay} Essay). DILARANG KERAS mengurangi jumlah soal!"#
        )
    } else {
        format!(
            r#"ATURAN KOMPOSISI SOAL CBT (FULL PILIHAN GANDA):
- Jumlah Soal Pilihan Ganda (PG): TEPAT {total_questions} butir ('question_type': 'MULTIPLE_CHOICE').
  Setiap butir soal memiliki 4 pilihan A, B, C, D dengan kunci jawaban correct_key ('A', 'B', 'C', atau 'D') dan pembahasan ilmiah.
- Total butir dalam array 'questions' WAJIB TEPAT {total_questions} butir. DILARANG KERAS hanya membuat 1 atau 2 soal!"#
        )
    };

    let sample_second_type = if is_mixed && num_essay > 0 {
        "ESSAY"
    } else {
        "MULTIPLE_CHOICE"
    };
    let sample_second_pts = if is_mixed && num_essay > 0 { 20 } else { 10 };

    let prompt = if is_english {
        format!(
            r#"You are a school CBT exam author designing official English assessments.
Create EXACTLY {total_questions} DISTINCT, DYNAMIC English questions for {grade_level} subject "{subject_name}" on topic: "{topic}", difficulty: {difficulty} (Batch ID: {now_ts}).

{subject_guideline}

{composition_instruction}
{visual_instructions}

CRITICAL REQUIREMENTS:
- EVERY text field (title, description, question_text, options A/B/C/D, and explanation) MUST be written 100% in natural English (ZERO Indonesian words).
- 'questions' MUST contain EXACTLY {total_questions} unique items.
- Distribute correct_key evenly across 'A', 'B', 'C', and 'D'.

Output MUST be pure JSON matching this schema:
{{
  "title": "CBT Question Set: {topic}",
  "description": "CBT assessment with {total_questions} English questions on {topic} ({subject_name}).",
  "questions": [
    {{
      "id": "q1",
      "question_text": "Clear contextual English question 1...",
      "question_type": "MULTIPLE_CHOICE",
      "options": {{
        "A": "Option A in English",
        "B": "Option B in English",
        "C": "Option C in English",
        "D": "Option D in English"
      }},
      "correct_key": "B",
      "explanation": "Concise English explanation of why B is correct...",
      "bloom_level": "C3",
      "points": 10
    }},
    {{
      "id": "q2",
      "question_text": "Next English question...",
      "question_type": "{sample_second_type}",
      "options": {{
        "A": "Option A in English",
        "B": "Option B in English",
        "C": "Option C in English",
        "D": "Option D in English"
      }},
      "correct_key": "C",
      "explanation": "English explanation or grading rubric...",
      "bloom_level": "C4",
      "points": {sample_second_pts}
    }}
  ]
}}

Output ONLY pure JSON with no extra text."#
        )
    } else {
        format!(
            r#"Anda adalah tim pembuat bank soal ujian CBT resmi sekolah (Kurikulum Merdeka).
Buatkan TEPAT {total_questions} butir soal yang BERBEDA, DINAMIS, dan BERVARIASI untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}" tingkat kesulitan: {difficulty} (ID Batch: {now_ts}).

{subject_guideline}

{composition_instruction}
{visual_instructions}

PENTING & WAJIB:
- Array 'questions' HARUS BERISI TEPAT {total_questions} BUTIR SOAL LENGKAP.
- Setiap soal harus unik, kontekstual, dan menguji aspek pemahaman yang mendalam dari materi {topic}.
- Kunci jawaban PG (correct_key) harus didistribusikan secara acak dan seimbang antara 'A', 'B', 'C', dan 'D'.

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Paket Soal CBT: {topic}",
  "description": "Ujian CBT {total_questions} butir soal materi {topic} ({subject_name}).",
  "questions": [
    {{
      "id": "q1",
      "question_text": "Teks butir soal nomor 1 yang jelas dan kontekstual...",
      "question_type": "MULTIPLE_CHOICE",
      "options": {{
        "A": "Pilihan A",
        "B": "Pilihan B",
        "C": "Pilihan C",
        "D": "Pilihan D"
      }},
      "correct_key": "B",
      "explanation": "Pembahasan lengkap kenapa B benar...",
      "bloom_level": "C3",
      "points": 10
    }},
    {{
      "id": "q2",
      "question_text": "Teks butir soal berikutnya...",
      "question_type": "{sample_second_type}",
      "options": {{
        "A": "Pilihan A",
        "B": "Pilihan B",
        "C": "Pilihan C",
        "D": "Pilihan D"
      }},
      "correct_key": "C",
      "explanation": "Pedoman jawaban dan rubrik penskoran...",
      "bloom_level": "C4",
      "points": {sample_second_pts}
    }}
  ]
}}

Output HANYA JSON tanpa teks pembuka atau penutup."#
        )
    };

    let system_prompt = if is_english {
        "You are a professional English CBT exam author producing 100% English question banks in strict valid JSON with zero Indonesian."
    } else {
        "You are a professional CBT exam author producing valid JSON question banks strictly respecting the subject matter rules and producing the exact requested count of distinct questions."
    };

    let messages = vec![
        ChatMessage {
            role: "system",
            content: system_prompt,
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = match call_nvidia_nim(messages, 0.65, 4500).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for CBT quiz: {e}. Utilizing pedagogical fallback.");
            let mut dto = fallback_generate_cbt_quiz(
                topic,
                grade_level,
                subject_name,
                quiz_format,
                num_mcq,
                num_essay,
                difficulty,
                include_images,
            );
            if include_images {
                ensure_quiz_visual_guarantee(
                    pool,
                    &mut dto.questions,
                    subject_name,
                    topic,
                    grade_level,
                    is_english,
                )
                .await;
            }
            return Ok(dto);
        }
    };
    let cleaned = extract_clean_json(&raw);
    let sanitized = sanitize_json_control_chars(cleaned);

    #[derive(Deserialize)]
    struct RawQuizResponse {
        title: Option<serde_json::Value>,
        description: Option<serde_json::Value>,
        #[serde(default)]
        questions: Vec<serde_json::Value>,
    }

    let parsed: RawQuizResponse = match serde_json::from_str(&sanitized) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse AI CBT Quiz JSON ({e}). Utilizing pedagogical fallback.");
            let mut dto = fallback_generate_cbt_quiz(
                topic,
                grade_level,
                subject_name,
                quiz_format,
                num_mcq,
                num_essay,
                difficulty,
                include_images,
            );
            if include_images {
                ensure_quiz_visual_guarantee(
                    pool,
                    &mut dto.questions,
                    subject_name,
                    topic,
                    grade_level,
                    is_english,
                )
                .await;
            }
            return Ok(dto);
        }
    };

    let default_pts = (100 / total_questions.max(1) as i32).max(5);
    let mut questions: Vec<CbtQuestionDto> = Vec::new();
    let mut diagrams_attached = 0usize;

    for (idx, q_val) in parsed.questions.into_iter().enumerate() {
        let q_text_raw = q_val
            .get("question_text")
            .or_else(|| q_val.get("text"))
            .map(json_value_to_string)
            .unwrap_or_default();
        let q_text = clean_question_stem(&q_text_raw);
        if q_text.trim().is_empty() {
            continue;
        }

        let explicit_type = q_val
            .get("question_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_uppercase());

        let mut choices: Vec<CbtQuestionChoiceDto> = Vec::new();
        let mut correct_key = q_val
            .get("correct_key")
            .and_then(|k| k.as_str())
            .unwrap_or("A")
            .trim()
            .to_uppercase();

        let raw_choices_val = q_val
            .get("options")
            .or_else(|| q_val.get("choices"))
            .or_else(|| q_val.get("pilihan"))
            .or_else(|| q_val.get("opsi"));

        if let Some(opts) = raw_choices_val {
            if let Some(map) = opts.as_object() {
                for (key, val) in map {
                    let k_upper = key.trim().to_uppercase();
                    let is_correct = k_upper == correct_key;
                    let clean_val = clean_choice_text(&json_value_to_string(val));
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: clean_val,
                        is_correct,
                    });
                }
            } else if let Some(arr) = opts.as_array() {
                for (c_idx, c_item) in arr.iter().enumerate() {
                    let letter = ((b'A' + (c_idx as u8).min(25)) as char).to_string();
                    let c_text_raw = if let Some(obj) = c_item.as_object() {
                        obj.get("choice_text")
                            .or_else(|| obj.get("text"))
                            .map(json_value_to_string)
                            .unwrap_or_else(|| json_value_to_string(c_item))
                    } else {
                        json_value_to_string(c_item)
                    };
                    let is_correct = if let Some(obj) = c_item.as_object() {
                        obj.get("is_correct")
                            .and_then(|b| b.as_bool())
                            .unwrap_or(letter == correct_key)
                    } else {
                        letter == correct_key
                    };
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: clean_choice_text(&c_text_raw),
                        is_correct,
                    });
                }
            }
        }

        // Ensure at least one choice is marked correct if multiple choice
        if !choices.is_empty() && !choices.iter().any(|c| c.is_correct) {
            let target_idx = match correct_key.as_str() {
                "B" if choices.len() > 1 => 1,
                "C" if choices.len() > 2 => 2,
                "D" if choices.len() > 3 => 3,
                _ => 0,
            };
            if let Some(c) = choices.get_mut(target_idx) {
                c.is_correct = true;
            }
        }

        let is_essay = explicit_type.as_deref() == Some("ESSAY")
            || (is_mixed && idx >= num_mcq)
            || choices.is_empty();

        let question_type = if is_essay {
            choices.clear();
            correct_key = String::new();
            "ESSAY".to_string()
        } else {
            "MULTIPLE_CHOICE".to_string()
        };

        let explanation = q_val
            .get("explanation")
            .map(json_value_to_string)
            .unwrap_or_default();
        let bloom_level = q_val
            .get("bloom_level")
            .and_then(|b| b.as_str())
            .unwrap_or("C3")
            .to_string();
        let points = q_val
            .get("points")
            .and_then(|p| p.as_i64())
            .map(|p| p as i32)
            .unwrap_or(if is_essay { 20 } else { default_pts });

        let img_url = if include_images && diagrams_attached < 3 {
            let resolved = crate::infrastructure::visual_diagram::resolve_visual_diagram_url(
                pool,
                q_val.get("diagram_spec"),
                subject_name,
                grade_level,
            )
            .await;
            if resolved.is_some() {
                diagrams_attached += 1;
            }
            resolved
        } else {
            None
        };
        let clean_q_text = if img_url.is_some() {
            strip_leaked_diagram_bracket_tag(&q_text)
        } else {
            q_text
        };

        questions.push(CbtQuestionDto {
            id: format!("gen-q-{}-{}", idx + 1, now_ts),
            question_text: clean_q_text,
            question_type: Some(question_type),
            image_url: img_url,
            choices,
            correct_key,
            explanation,
            bloom_level,
            points,
        });
    }

    // ── Enforce Exact Counts for CBT Quiz ──
    if is_mixed {
        let mut final_mcqs: Vec<CbtQuestionDto> = Vec::new();
        let mut final_essays: Vec<CbtQuestionDto> = Vec::new();

        for q in questions {
            if q.question_type.as_deref() == Some("MULTIPLE_CHOICE") && !q.choices.is_empty() {
                final_mcqs.push(q);
            } else {
                final_essays.push(q);
            }
        }

        if final_mcqs.len() < num_mcq {
            let needed = num_mcq - final_mcqs.len();
            let key_labels = ["A", "B", "C", "D"];
            let pad_templates = generate_pedagogical_question_templates(
                subject_name,
                topic,
                grade_level,
                difficulty,
                num_mcq,
            );
            let start_idx = final_mcqs.len();
            for offset in 0..needed {
                let tpl = &pad_templates[(start_idx + offset) % pad_templates.len()];
                let idx = final_mcqs.len() + 1;
                let choices = tpl
                    .1
                    .iter()
                    .enumerate()
                    .map(|(c_idx, text)| CbtQuestionChoiceDto {
                        choice_text: text.clone(),
                        is_correct: c_idx == tpl.2,
                    })
                    .collect();
                final_mcqs.push(CbtQuestionDto {
                    id: format!("cbt-mcq-pad-{idx}-{now_ts}"),
                    question_text: tpl.0.clone(),
                    question_type: Some("MULTIPLE_CHOICE".to_string()),
                    image_url: None,
                    choices,
                    correct_key: key_labels[tpl.2 % 4].to_string(),
                    explanation: tpl.3.clone(),
                    bloom_level: "C3".to_string(),
                    points: default_pts,
                });
            }
        }

        while final_essays.len() < num_essay {
            let idx = final_essays.len() + 1;
            final_essays.push(CbtQuestionDto {
                id: format!("cbt-essay-pad-{idx}-{now_ts}"),
                question_text: if is_english {
                    format!("Analyze and write a clear explanation in English showing how '{topic}' ({subject_name}) applies in real-life communication (Essay {idx}):")
                } else {
                    format!("Analisis dan uraikan pemahaman Anda mengenai penerapan materi {topic} dalam kehidupan sehari-hari (Soal Uraian {idx}):")
                },
                question_type: Some("ESSAY".to_string()),
                image_url: None,
                choices: Vec::new(),
                correct_key: String::new(),
                explanation: if is_english {
                    format!("Rubric: Full points awarded for accurate English concepts on {topic}, clear supporting examples, and grammatical accuracy.")
                } else {
                    format!("Rubrik: Skor maksimal jika siswa mampu menjabarkan konsep {topic} secara sistematis dengan bukti/contoh logis.")
                },
                bloom_level: "C4".to_string(),
                points: 20,
            });
        }

        final_mcqs.truncate(num_mcq);
        final_essays.truncate(num_essay);

        questions = final_mcqs;
        questions.extend(final_essays);
    } else {
        if questions.len() < total_questions {
            let needed = total_questions - questions.len();
            let key_labels = ["A", "B", "C", "D"];
            let pad_templates = generate_pedagogical_question_templates(
                subject_name,
                topic,
                grade_level,
                difficulty,
                total_questions,
            );
            let start_idx = questions.len();
            for offset in 0..needed {
                let tpl = &pad_templates[(start_idx + offset) % pad_templates.len()];
                let idx = questions.len() + 1;
                let choices = tpl
                    .1
                    .iter()
                    .enumerate()
                    .map(|(c_idx, text)| CbtQuestionChoiceDto {
                        choice_text: text.clone(),
                        is_correct: c_idx == tpl.2,
                    })
                    .collect();
                questions.push(CbtQuestionDto {
                    id: format!("cbt-mcq-pad-{idx}-{now_ts}"),
                    question_text: tpl.0.clone(),
                    question_type: Some("MULTIPLE_CHOICE".to_string()),
                    image_url: None,
                    choices,
                    correct_key: key_labels[tpl.2 % 4].to_string(),
                    explanation: tpl.3.clone(),
                    bloom_level: "C3".to_string(),
                    points: default_pts,
                });
            }
        }
        questions.truncate(total_questions);
    }

    if include_images {
        ensure_quiz_visual_guarantee(
            pool,
            &mut questions,
            subject_name,
            topic,
            grade_level,
            is_english,
        )
        .await;
    }

    let title_str = parsed
        .title
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_english {
                format!("CBT Question Set: {topic}")
            } else {
                format!("Paket Soal CBT: {topic}")
            }
        });
    let desc_str = parsed
        .description
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_english {
                format!("CBT assessment with {total_questions} questions on {topic} ({subject_name}).")
            } else {
                format!("Ujian CBT {total_questions} butir soal materi {topic} ({subject_name}).")
            }
        });

    Ok(GeneratedCbtQuizDto {
        title: title_str,
        description: desc_str,
        questions,
    })
}

pub fn fallback_generate_cbt_quiz(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    quiz_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    _include_images: bool,
) -> GeneratedCbtQuizDto {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let is_english = is_english_subject(subject_name);
    let is_mixed = num_essay > 0
        || quiz_format
            .map(|f| f.to_uppercase() == "MIXED" || f.to_uppercase().contains("CAMPURAN"))
            .unwrap_or(false);
    let total_questions = if is_mixed { num_mcq + num_essay } else { num_mcq };
    let default_pts = (100 / total_questions.max(1) as i32).max(5);

    let templates = generate_pedagogical_question_templates(subject_name, topic, grade_level, difficulty, num_mcq);
    let key_labels = ["A", "B", "C", "D"];
    let bloom_levels = ["C2", "C3", "C4", "C3", "C4"];

    let mut questions = Vec::new();

    for (idx, (q_text, choices_text, correct_idx, expl)) in templates.into_iter().enumerate() {
        let choices = choices_text
            .into_iter()
            .enumerate()
            .map(|(c_idx, text)| CbtQuestionChoiceDto {
                choice_text: text,
                is_correct: c_idx == correct_idx,
            })
            .collect();

        questions.push(CbtQuestionDto {
            id: format!("cbt-q-{}-{now_ts}", idx + 1),
            question_text: q_text,
            question_type: Some("MULTIPLE_CHOICE".to_string()),
            image_url: None,
            choices,
            correct_key: key_labels[correct_idx % 4].to_string(),
            explanation: expl,
            bloom_level: bloom_levels[idx % bloom_levels.len()].to_string(),
            points: default_pts,
        });
    }

    if is_mixed && num_essay > 0 {
        let essay_prompts = if is_english {
            [
                format!("Analyze and explain clearly in English how key concepts of '{topic}' ({subject_name}) apply to everyday communication for {grade_level} students!"),
                format!("Write a short English dialogue or paragraph demonstrating accurate use of vocabulary and sentence patterns from '{topic}'."),
                format!("Reflect in English on the main ideas of '{topic}' and summarize two practical communication strategies you can use."),
            ]
        } else {
            [
                format!("Analisis dan uraikan secara mendalam bagaimana prinsip utama '{topic}' ({subject_name}) diterapkan dalam memecahkan masalah nyata bagi siswa {grade_level}!"),
                format!("Berdasarkan materi '{topic}', jelaskan hubungan sebab-akibat antar-komponen utama dan berikan satu studi kasus konkret beserta pembahasannya!"),
                format!("Evaluasilah kelebihan dan tantangan dalam penerapan konsep '{topic}' serta rumuskan rekomendasi solusi yang inovatif dan terukur!"),
            ]
        };

        for idx in 0..num_essay {
            let prompt = essay_prompts[idx % essay_prompts.len()].clone();
            let q_num = questions.len() + 1;
            questions.push(CbtQuestionDto {
                id: format!("cbt-essay-{}-{now_ts}", idx + 1),
                question_text: if num_essay > essay_prompts.len() {
                    if is_english {
                        format!("(Essay {q_num}) {prompt}")
                    } else {
                        format!("(Uraian {q_num}) {prompt}")
                    }
                } else {
                    prompt
                },
                question_type: Some("ESSAY".to_string()),
                image_url: None,
                choices: Vec::new(),
                correct_key: "ESSAY".to_string(),
                explanation: if is_english {
                    "Scoring Rubric: Full score awarded for accurate English expression, strong comprehension, and coherent supporting details.".to_string()
                } else {
                    "Rubrik Penskoran: Skor penuh diberikan bila jawaban memuat analisis konsep akurat, argumentasi logis runtut, dan bukti/contoh aplikatif yang valid.".to_string()
                },
                bloom_level: "C4".to_string(),
                points: 20,
            });
        }
    }

    GeneratedCbtQuizDto {
        title: if is_english {
            format!("CBT Question Set: {topic}")
        } else {
            format!("Paket Soal CBT: {topic}")
        },
        description: if is_english {
            format!("CBT exam with {total_questions} English questions on {topic} ({subject_name}), {grade_level} ({difficulty}).")
        } else {
            format!("Ujian CBT {total_questions} butir soal materi {topic} ({subject_name}) jenjang {grade_level} ({difficulty}).")
        },
        questions,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. STUDENT & AT-RISK ANALYTICS DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StudentAnalyticsInsightDto {
    pub student_id: Uuid,
    pub student_name: String,
    pub summary: String,
    pub strengths: Vec<String>,
    pub areas_for_improvement: Vec<String>,
    pub recommendations: Vec<String>,
    pub risk_level: String, // "RENDAH" | "SEDANG" | "TINGGI"
}

pub async fn generate_student_analytics(
    student_id: Uuid,
    student_name: &str,
    grade_level: &str,
    attendance_pct: f64,
    avg_score: f64,
    uncompleted_tasks: i64,
) -> Result<StudentAnalyticsInsightDto, ApplicationError> {
    let prompt = format!(
        r#"Anda adalah asisten konselor akademik AI berbasis analitik data siswa.
Analisis profil siswa berikut:
- Nama: {student_name}
- Jenjang: {grade_level}
- Tingkat Kehadiran: {attendance_pct:.1}%
- Rata-rata Nilai Kuis/Ujian: {avg_score:.1} / 100
- Tugas/Modul Belum Selesai: {uncompleted_tasks} tugas

Buatkan diagnosis analitik & rekomendasi tindakan untuk wali kelas dan orang tua.
Format output HARUS berupa JSON murni dengan skema:
{{
  "summary": "Ringkasan kondisi performa akademik siswa 2 kalimat.",
  "strengths": ["Kekuatan 1", "Kekuatan 2"],
  "areas_for_improvement": ["Hal yang perlu ditingkatkan 1", "Hal yang perlu ditingkatkan 2"],
  "recommendations": ["Saran tindakan guru/ortu 1", "Saran tindakan 2"],
  "risk_level": "RENDAH / SEDANG / TINGGI"
}}

Output HANYA JSON tanpa teks lain."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an educational analytics counselor producing strict JSON insights.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.3, 1200).await?;
    let cleaned = extract_clean_json(&raw);

    #[derive(Deserialize)]
    struct RawAnalytics {
        summary: String,
        strengths: Vec<String>,
        areas_for_improvement: Vec<String>,
        recommendations: Vec<String>,
        risk_level: String,
    }

    let parsed: RawAnalytics = serde_json::from_str(cleaned).map_err(|e| {
        ApplicationError::Domain(DomainError::Validation(format!(
            "Invalid AI Analytics JSON: {e}"
        )))
    })?;

    Ok(StudentAnalyticsInsightDto {
        student_id,
        student_name: student_name.to_string(),
        summary: parsed.summary,
        strengths: parsed.strengths,
        areas_for_improvement: parsed.areas_for_improvement,
        recommendations: parsed.recommendations,
        risk_level: parsed.risk_level.to_uppercase(),
    })
}

/// Parses raw exam text (extracted from PDF / Word / TXT) into structured CBT quiz questions & answer keys.
pub async fn parse_quiz_document_from_text(
    raw_text: &str,
    file_name: Option<&str>,
    subject_hint: Option<&str>,
    grade_level_hint: Option<&str>,
) -> Result<GeneratedCbtQuizDto, ApplicationError> {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return Err(ApplicationError::Domain(DomainError::Validation(
            "Teks naskah soal kosong".to_string(),
        )));
    }

    // Safely truncate at valid UTF-8 boundary around 22,000 bytes so request stays within model context.
    let truncated = if trimmed.len() > 22_000 {
        let mut end = 22_000;
        while end > 0 && !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        &trimmed[..end]
    } else {
        trimmed
    };

    let fn_hint = file_name.unwrap_or("naskah-ujian");
    let sub_hint = subject_hint.unwrap_or("Umum");
    let grade_hint = grade_level_hint.unwrap_or("Semua Jenjang");

    let prompt = format!(
        r#"Anda adalah parser naskah soal ujian CBT profesional Indonesia.
Tugas Anda: ekstrak seluruh butir soal beserta pilihan jawaban (jika pilihan ganda) dan KUNCI JAWABAN secara akurat dari teks dokumen berikut.

Metadata opsional:
- Nama file: {fn_hint}
- Mata pelajaran: {sub_hint}
- Jenjang/Kelas: {grade_hint}

ATURAN EKSTRAKSI KETAT:
1. Jangan mengarang soal baru; ekstrak hanya soal yang benar-benar ada di teks dokumen!
2. Jika ada bagian "KUNCI JAWABAN" di awal/tengah/akhir dokumen atau tabel kunci, cocokkan kunci tersebut ke nomor soal yang sesuai, lalu jangan masukkan blok daftar kunci menjadi teks soal.
3. Jika ada penanda seperti `[CORRECT_HINT]`, `(BENAR)`, `Kunci: A`, `Jawaban: C`, atau `*A.` pada opsi, tandai pilihan yang sesuai dengan `is_correct: true` dan isi `correct_key` (mis. "A", "B", "C", "D", atau "E"), lalu bersihkan penanda tersebut dari teks opsi/soal.
4. Bersihkan awalan huruf opsi ("A. ", "B) ") dari `choice_text`.
5. Untuk soal Uraian/Essay (tanpa opsi A-E), isi `question_type: "ESSAY"`, `choices: []`, dan isi `correct_key` dengan ringkasan kunci/pedoman penskoran jika tersedia.
6. Bagi poin secara wajar atau gunakan bobot dari naskah jika tertulis (mis. total mendekati 100 atau default 10).

Format output WAJIB berupa JSON murni tanpa markdown/komentar tambahan:
{{
  "title": "Judul ujian dari dokumen atau nama file",
  "description": "Deskripsi singkat mata pelajaran ujian",
  "questions": [
    {{
      "id": "q-doc-1",
      "question_text": "Pertanyaan lengkap soal",
      "question_type": "MULTIPLE_CHOICE",
      "choices": [
        {{ "choice_text": "Pilihan A", "is_correct": false }},
        {{ "choice_text": "Pilihan B", "is_correct": true }}
      ],
      "correct_key": "B",
      "explanation": "Pembahasan/kunci jika tertulis di dokumen atau kosong",
      "bloom_level": "C2",
      "points": 10
    }}
  ]
}}

TEKS DOKUMEN NASKAH SOAL:
\"\"\"
{truncated}
\"\"\"

Output HANYA JSON valid."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are a precise exam document parser that extracts existing Indonesian school questions, options, and answer keys into strict valid JSON.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.1, 4500).await?;
    let cleaned = extract_clean_json(&raw);

    let json_candidate = if let Ok(v) = serde_json::from_str::<GeneratedCbtQuizDto>(cleaned) {
        v
    } else if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        serde_json::from_str::<GeneratedCbtQuizDto>(&cleaned[start..=end]).map_err(|e| {
            ApplicationError::Domain(DomainError::Validation(format!(
                "AI mengembalikan struktur JSON parser soal tidak valid: {e}"
            )))
        })?
    } else {
        return Err(ApplicationError::Domain(DomainError::Validation(
            "AI tidak menghasilkan JSON hasil ekstraksi soal.".to_string(),
        )));
    };

    let now_ts = chrono::Utc::now().timestamp_millis();
    let total_q = json_candidate.questions.len().max(1);
    let default_pts = ((100.0 / total_q as f64).round() as i32).max(1);

    let normalized_questions: Vec<CbtQuestionDto> = json_candidate
        .questions
        .into_iter()
        .enumerate()
        .filter(|(_, q)| !q.question_text.trim().is_empty())
        .map(|(idx, mut q)| {
            if q.id.trim().is_empty() {
                q.id = format!("q-doc-{}-{now_ts}", idx + 1);
            }
            if q.points <= 0 {
                q.points = default_pts;
            }
            if q.bloom_level.trim().is_empty() {
                q.bloom_level = "C2".to_string();
            }
            let is_essay = q.choices.is_empty()
                || q.question_type
                    .as_deref()
                    .map(|t| t.eq_ignore_ascii_case("ESSAY"))
                    .unwrap_or(false);
            if is_essay {
                q.question_type = Some("ESSAY".to_string());
                q.choices.clear();
            } else {
                q.question_type = Some("MULTIPLE_CHOICE".to_string());
                let key_upper = q.correct_key.trim().to_uppercase();
                let key_idx = key_upper
                    .chars()
                    .next()
                    .and_then(|c| ('A'..='E').position(|letter| letter == c));

                let already_has_true = q.choices.iter().any(|c| c.is_correct);
                if !already_has_true {
                    if let Some(k_idx) = key_idx {
                        if let Some(choice) = q.choices.get_mut(k_idx) {
                            choice.is_correct = true;
                        }
                    }
                } else if q.correct_key.trim().is_empty() {
                    if let Some(pos) = q.choices.iter().position(|c| c.is_correct) {
                        if let Some(letter) = ['A', 'B', 'C', 'D', 'E'].get(pos) {
                            q.correct_key = letter.to_string();
                        }
                    }
                }
            }
            q
        })
        .collect();

    Ok(GeneratedCbtQuizDto {
        title: if json_candidate.title.trim().is_empty() {
            format!("Ujian Impor: {fn_hint}")
        } else {
            json_candidate.title
        },
        description: if json_candidate.description.trim().is_empty() {
            format!("Hasil ekstraksi otomatis dari dokumen {fn_hint}")
        } else {
            json_candidate.description
        },
        questions: normalized_questions,
    })
}
