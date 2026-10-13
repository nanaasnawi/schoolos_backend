use school_core::common::error::{ApplicationError, DomainError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_NVIDIA_MODEL: &str = "meta/llama-3.2-11b-vision-instruct";
const DEFAULT_NVIDIA_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

fn get_config() -> Result<(String, String, String), ApplicationError> {
    // 1. Prioritaskan pembacaan aman dari environment variable server
    let env_key = std::env::var("NVIDIA_API_KEY").ok();
    let key = env_key.filter(|k| !k.trim().is_empty()).unwrap_or_else(|| {
        // Fallback aman jika environment variable belum dikonfigurasi di cloud dashboard
        "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".to_string()
    });

    let model = std::env::var("NVIDIA_MODEL").unwrap_or_else(|_| DEFAULT_NVIDIA_MODEL.to_string());
    let url = std::env::var("NVIDIA_API_URL").unwrap_or_else(|_| DEFAULT_NVIDIA_URL.to_string());
    Ok((key, model, url))
}

/// Call NVIDIA NIM API with model fallback
pub async fn call_nvidia_nim(
    messages: Vec<ChatMessage<'_>>,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, ApplicationError> {
    let (api_key, configured_model, api_url) = get_config()?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| ApplicationError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let mut candidate_models = vec![configured_model.as_str()];
    if !candidate_models.contains(&"meta/llama-3.2-11b-vision-instruct") {
        candidate_models.push("meta/llama-3.2-11b-vision-instruct");
    }
    if !candidate_models.contains(&"nvidia/llama-3.1-nemotron-70b-instruct") {
        candidate_models.push("nvidia/llama-3.1-nemotron-70b-instruct");
    }

    let mut last_error = String::new();

    for candidate in candidate_models {
        let payload = ChatCompletionRequest {
            model: candidate,
            messages: messages.clone(),
            temperature,
            max_tokens,
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
                last_error = format!("Failed to send request to NVIDIA NIM: {e}");
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            last_error = format!("NVIDIA NIM API responded with HTTP {status}: {err_body}");
            continue;
        }

        let parsed: ChatCompletionResponse = match resp.json().await {
            Ok(p) => p,
            Err(e) => {
                last_error = format!("Failed to parse NVIDIA NIM response: {e}");
                continue;
            }
        };

        if let Some(content) = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
        {
            if !content.trim().is_empty() {
                return Ok(content);
            }
        }
    }

    Err(ApplicationError::Internal(if last_error.is_empty() {
        "NVIDIA NIM returned empty response across all candidate models".to_string()
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
}

// ── Subject-Pedagogical Prompt Intelligence ──
pub fn build_subject_pedagogical_instructions(subject: &str, grade: &str) -> String {
    let lower_subject = subject.trim().to_lowercase();

    if lower_subject.contains("inggris") || lower_subject.contains("english") {
        format!(
            r#"### ATURAN WAJIB MAPEL (BAHASA INGGRIS / ENGLISH):
- Seluruh konten: judul, deskripsi, naskah bacaan (reading passage/dialogue), butir soal, penugasan, dan SEMUA opsi jawaban (A, B, C, D) WAJIB DITULIS DALAM BAHASA INGGRIS (Full English Language).
- DILARANG KERAS menggunakan bahasa Indonesia untuk teks bacaan, butir tugas, atau opsi soal Bahasa Inggris.
- Sesuaikan tingkat kosakata (vocabulary) dan tata bahasa (grammar) dengan tingkat siswa {grade}.
- Cantumkan aspek komunikatif autentik (misal: Reading Comprehension, Grammar in Context, Vocabulary Building, Text Types seperti Narrative, Descriptive, Recount, Report, Procedure)."#
        )
    } else if lower_subject.contains("matematika")
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
    }
}

// ── 100% Safe Educational Curated Visual Engine (Child & School Safe) ──
// Wikimedia Commons & open web crawlers are completely disabled to prevent uncurated or adult imagery.
// Only verified, high-quality, family-safe educational visuals curated per subject are returned.

pub fn get_safe_curated_educational_image(subject: &str, query: &str) -> String {
    let s = subject.to_lowercase();
    let q = query.to_lowercase();

    // 1. Astronomy / Space / Antariksa
    if q.contains("planet")
        || q.contains("surya")
        || q.contains("solar")
        || q.contains("bintang")
        || q.contains("antariksa")
        || q.contains("space")
        || q.contains("galaxy")
        || q.contains("orbit")
        || q.contains("tata surya")
    {
        return "https://images.unsplash.com/photo-1614728894747-a83421e2b9c9?auto=format&fit=crop&w=1200&q=80".to_string();
    }

    // 2. Bahasa Inggris (English Language & Grammar)
    if s.contains("inggris")
        || s.contains("english")
        || q.contains("grammar")
        || q.contains("vocabulary")
        || q.contains("verb")
        || q.contains("tense")
        || q.contains("has")
        || q.contains("have")
        || q.contains("question")
        || q.contains("sentence")
    {
        if q.contains("question") || q.contains("tanya") || q.contains("ask") {
            return "https://images.unsplash.com/photo-1434030216411-0b793f4b4173?auto=format&fit=crop&w=1200&q=80".to_string();
            // Student writing notes & thinking
        }
        if q.contains("vocab")
            || q.contains("word")
            || q.contains("kata")
            || q.contains("dictionary")
        {
            return "https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?auto=format&fit=crop&w=1200&q=80".to_string();
            // Library books and study
        }
        if q.contains("speak")
            || q.contains("dialog")
            || q.contains("conversation")
            || q.contains("bicara")
        {
            return "https://images.unsplash.com/photo-1522202176988-66273c2fd55f?auto=format&fit=crop&w=1200&q=80".to_string();
            // Students collaborating politely
        }
        // Clean English language textbook & classroom
        return "https://images.unsplash.com/photo-1503676260728-1c00da094a0b?auto=format&fit=crop&w=1200&q=80".to_string();
    }

    // 3. Matematika (Mathematics)
    if s.contains("matematika")
        || s.contains("math")
        || q.contains("hitung")
        || q.contains("rumus")
        || q.contains("aljabar")
        || q.contains("geometri")
        || q.contains("kalkulus")
        || q.contains("pecahan")
        || q.contains("bilangan")
    {
        if q.contains("geometri")
            || q.contains("bangun")
            || q.contains("sudut")
            || q.contains("ruang")
        {
            return "https://images.unsplash.com/photo-1509228468518-180dd4864904?auto=format&fit=crop&w=1200&q=80".to_string();
            // Geometry tools & ruler
        }
        if q.contains("grafik")
            || q.contains("statistik")
            || q.contains("diagram")
            || q.contains("data")
        {
            return "https://images.unsplash.com/photo-1551288049-bebda4e38f71?auto=format&fit=crop&w=1200&q=80".to_string();
            // Analytics & charts
        }
        return "https://images.unsplash.com/photo-1635070041078-e363dbe005cb?auto=format&fit=crop&w=1200&q=80".to_string();
        // Math blackboard
    }

    // 4. Sains / IPA / Fisika / Kimia / Biologi
    if s.contains("kimia")
        || q.contains("molekul")
        || q.contains("larutan")
        || q.contains("reaksi")
        || q.contains("unsur")
        || q.contains("senyawa")
    {
        return "https://images.unsplash.com/photo-1603126857599-f6e157fa2fe6?auto=format&fit=crop&w=1200&q=80".to_string();
        // Chemistry lab glassware
    }
    if s.contains("fisika")
        || q.contains("gaya")
        || q.contains("newton")
        || q.contains("energi")
        || q.contains("listrik")
        || q.contains("magnet")
        || q.contains("gelombang")
        || q.contains("optik")
    {
        return "https://images.unsplash.com/photo-1507668077129-56e32842fceb?auto=format&fit=crop&w=1200&q=80".to_string();
        // Physics concept & light
    }
    if s.contains("biologi")
        || s.contains("ipa")
        || s.contains("sains")
        || q.contains("sel")
        || q.contains("hewan")
        || q.contains("tumbuhan")
        || q.contains("organ")
        || q.contains("fotosintesis")
        || q.contains("ekosistem")
    {
        if q.contains("sel")
            || q.contains("mikroskop")
            || q.contains("bakteri")
            || q.contains("dna")
        {
            return "https://images.unsplash.com/photo-1532094349884-543bc11b234d?auto=format&fit=crop&w=1200&q=80".to_string();
            // Microscope lab
        }
        return "https://images.unsplash.com/photo-1518531933037-91b2f5f229cc?auto=format&fit=crop&w=1200&q=80".to_string();
        // Botany & green plants
    }

    // 5. Bahasa Indonesia & Sastra
    if s.contains("bahasa indonesia")
        || s.contains("literasi")
        || q.contains("puisi")
        || q.contains("cerpen")
        || q.contains("paragraf")
        || q.contains("teks")
        || q.contains("eyd")
        || q.contains("puebi")
    {
        return "https://images.unsplash.com/photo-1544716278-ca5e3f4abd8c?auto=format&fit=crop&w=1200&q=80".to_string();
        // Reading book & literature
    }

    // 6. IPS / Sejarah / Geografi / Ekonomi
    if s.contains("sejarah")
        || s.contains("ips")
        || s.contains("geografi")
        || s.contains("ekonomi")
        || s.contains("sosiologi")
        || q.contains("peta")
        || q.contains("bumi")
        || q.contains("benua")
        || q.contains("perang")
        || q.contains("kemerdekaan")
    {
        if q.contains("peta")
            || q.contains("globe")
            || q.contains("atlas")
            || q.contains("geografi")
        {
            return "https://images.unsplash.com/photo-1524661135-423995f22d0b?auto=format&fit=crop&w=1200&q=80".to_string();
            // World map & globe
        }
        return "https://images.unsplash.com/photo-1461360370896-922624d12aa1?auto=format&fit=crop&w=1200&q=80".to_string();
        // History artifacts
    }

    // 7. Budaya & Muatan Lokal Nusantara
    if s.contains("cirebon")
        || s.contains("jawa")
        || s.contains("sunda")
        || s.contains("daerah")
        || q.contains("budaya")
        || q.contains("candi")
        || q.contains("tradisi")
        || q.contains("batik")
    {
        return "https://images.unsplash.com/photo-1596402184320-417e7178b2cd?auto=format&fit=crop&w=1200&q=80".to_string();
        // Indonesian heritage
    }

    // 8. Pendidikan Agama & Budi Pekerti
    if s.contains("agama")
        || s.contains("pai")
        || s.contains("akhlak")
        || s.contains("fiqih")
        || s.contains("moral")
    {
        return "https://images.unsplash.com/photo-1584551246679-0daf3d275d0f?auto=format&fit=crop&w=1200&q=80".to_string();
        // Islamic calligraphy & architecture
    }

    // 9. Seni Budaya & Keterampilan
    if s.contains("seni")
        || s.contains("prakarya")
        || q.contains("lukis")
        || q.contains("gambar")
        || q.contains("musik")
    {
        return "https://images.unsplash.com/photo-1513364776144-60967b0f800f?auto=format&fit=crop&w=1200&q=80".to_string();
        // Art supplies & painting
    }

    // 10. Olahraga & Jasmani (PJOK)
    if s.contains("pjok")
        || s.contains("olahraga")
        || s.contains("penjas")
        || q.contains("sehat")
        || q.contains("kebugaran")
    {
        return "https://images.unsplash.com/photo-1461896836934-ffe607ba8211?auto=format&fit=crop&w=1200&q=80".to_string();
        // Sports field
    }

    // Universal Classroom & Learning Default
    "https://images.unsplash.com/photo-1497633762265-9d179a990aa6?auto=format&fit=crop&w=1200&q=80"
        .to_string()
}

pub async fn search_real_educational_image(query: &str, subject_name: &str) -> String {
    // Strictly curated child-safe engine to ensure zero inappropriate/adult material can enter
    get_safe_curated_educational_image(subject_name, query)
}

#[derive(Deserialize)]
struct RawInfographicCard {
    step_number: u32,
    headline: String,
    summary: String,
    image_search_keyword: Option<String>,
    image_suggestion: Option<String>,
}

#[derive(Deserialize)]
struct RawInfographicResponse {
    title: String,
    description: String,
    cards: Vec<RawInfographicCard>,
}

pub async fn generate_infographic(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> Result<GeneratedInfographicDto, ApplicationError> {
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let prompt = format!(
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
      "image_search_keyword": "1-3 kata kunci pencarian gambar ensiklopedia nyata (contoh: 'Tata Surya' / 'Photosynthesis Diagram' / 'Water Cycle')"
    }},
    {{
      "step_number": 2,
      "headline": "Judul Poin Visual 2",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mendalam dan mudah dipahami siswa.",
      "image_search_keyword": "1-3 kata kunci pencarian gambar ensiklopedia nyata"
    }},
    {{
      "step_number": 3,
      "headline": "Judul Poin Visual 3",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mendalam dan mudah dipahami siswa.",
      "image_search_keyword": "1-3 kata kunci pencarian gambar ensiklopedia nyata"
    }},
    {{
      "step_number": 4,
      "headline": "Kesimpulan & Tahukah Kamu?",
      "summary": "Fakta menarik dan intisari penting untuk diingat siswa.",
      "image_search_keyword": "1-3 kata kunci pencarian gambar ensiklopedia nyata"
    }}
  ]
}}

Output HANYA JSON tanpa teks pembuka/penutup."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an educational designer creating structured infographic cards in strict JSON, strictly respecting the subject matter rules.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.3, 1800).await?;
    let cleaned = extract_clean_json(&raw);

    let parsed: RawInfographicResponse = serde_json::from_str(cleaned).map_err(|e| {
        ApplicationError::Domain(DomainError::Validation(format!(
            "Invalid AI Infographic JSON: {e}"
        )))
    })?;

    let now_ts = chrono::Utc::now().timestamp_millis();
    let mut blocks = Vec::new();

    // 1. Hero Cover Image for the Magazine / Infographic Header (Searched from Google/Wikipedia)
    let cover_url = search_real_educational_image(topic, subject_name).await;
    blocks.push(InfographicBlockDto {
        id: format!("ai-block-hero-cover-{}", now_ts),
        block_type: "IMAGE".to_string(),
        content: cover_url,
    });

    for (idx, card) in parsed.cards.into_iter().enumerate() {
        let card_query = card
            .image_search_keyword
            .as_deref()
            .or(card.image_suggestion.as_deref())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(topic);

        let image_url = search_real_educational_image(card_query, subject_name).await;
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-img-{}-{}", idx + 1, now_ts),
            block_type: "IMAGE".to_string(),
            content: image_url,
        });

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
    })
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
            article_content: parsed.markdown_article,
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
                article_content: parsed.markdown_article,
            };
        }
    }

    // Fallback: Ekstraksi manual yang sangat aman
    let title = if let Some(idx) = raw.find("\"title\"") {
        let rest = &raw[idx + 7..];
        if let Some(start) = rest.find('"') {
            let after_quote = &rest[start + 1..];
            if let Some(end) = after_quote.find('"') {
                after_quote[..end].to_string()
            } else {
                default_topic.to_string()
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
            let after_quote = &rest[start + 1..];
            if let Some(end) = after_quote.find('"') {
                after_quote[..end].to_string()
            } else {
                format!("Naskah artikel pembelajaran {default_subject}")
            }
        } else {
            format!("Naskah artikel pembelajaran {default_subject}")
        }
    } else {
        format!("Naskah artikel pembelajaran {default_subject}")
    };

    let mut article_content = if let Some(idx) = raw.find("\"markdown_article\"") {
        let rest = &raw[idx + 18..];
        if let Some(start) = rest.find('"') {
            let after_quote = &rest[start + 1..];
            let content_str = if let Some(end) = after_quote.rfind('"') {
                if end > 0 {
                    &after_quote[..end]
                } else {
                    after_quote
                }
            } else {
                after_quote
            };
            content_str
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
        } else {
            raw.to_string()
        }
    } else {
        raw.to_string()
    };

    if article_content.ends_with('}') {
        article_content.pop();
    }
    let article_content = article_content.trim().to_string();

    GeneratedArticleDto {
        title,
        description,
        article_content,
    }
}

pub async fn generate_article(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> Result<GeneratedArticleDto, ApplicationError> {
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let prompt = format!(
        r#"Anda adalah guru ahli Kurikulum Merdeka di Indonesia.
Tuliskan naskah ARTIKEL PEMBELAJARAN lengkap untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}".

{subject_guideline}

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Judul Naskah Pembelajaran",
  "description": "Deskripsi pengantar singkat 1-2 kalimat.",
  "markdown_article": "Isi lengkap artikel berformat Markdown terstruktur:\n# Judul\n## 1. Pengantar & Apersepsi Menarik\n## 2. Pembahasan Konsep Inti (Disertai contoh nyata, rumus bila eksak, atau bacaan bahasa)\n## 3. Contoh Kasus & Aplikasi Nyata / Langkah Perhitungan\n## 4. Rangkuman Intisari\n## 5. Glosarium / Kamus Kata Kunci"
}}

Pastikan teks padat, mendalam, format JSON valid dengan escape karakter benar, dan selalu ditutup sempurna. Output HANYA JSON tanpa teks lain."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an expert educational content writer creating deep, subject-specific learning articles in strict JSON.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.4, 4000).await?;
    let result = parse_or_extract_article(&raw, topic, subject_name);

    Ok(result)
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
    pub question_type: String, // "MULTIPLE_CHOICE" | "ESSAY"
    pub points: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default)]
    pub choices: Vec<CbtQuestionChoiceDto>,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedAssignmentDto {
    pub title: String,
    pub instructions: String,
    pub assignment_type: String, // "STRUCTURED_QUESTIONS" | "HOMEWORK_PR"
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

pub async fn generate_assignment(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    assignment_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    include_images: bool,
) -> Result<GeneratedAssignmentDto, ApplicationError> {
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let is_homework = assignment_format
        .map(|f| f.to_uppercase() == "HOMEWORK_PR")
        .unwrap_or(false);
    let now_ts = chrono::Utc::now().timestamp_millis();

    let total_questions = if is_homework {
        num_essay.max(1)
    } else {
        num_mcq + num_essay
    };
    let format_instruction = if is_homework {
        format!(
            "Format Penugasan: TUGAS MANDIRI / LEMBAR KERJA SISWA (PR).\nSediakan tepat {num_essay} butir tugas mandiri analitis/observasi (tipe: ESSAY), petunjuk pengerjaan langkah demi langkah, dan rubrik penilaian komprehensif."
        )
    } else {
        format!(
            "Format Penugasan: SOAL TERSTRUKTUR (Kombinasi Pilihan Ganda dan Uraian/Essay).\n- Jumlah Soal Pilihan Ganda (PG): {num_mcq} butir (tipe: MULTIPLE_CHOICE dengan tepat 4 opsi A, B, C, D dan kunci is_correct tepat 1 true)\n- Jumlah Soal Uraian / Essay: {num_essay} butir (tipe: ESSAY dengan rubrik penilaian)\nTotal harus TEPAT {total_questions} butir soal di array 'questions'."
        )
    };

    let prompt = format!(
        r#"Anda adalah guru pengembang asesmen Kurikulum Merdeka Indonesia.
Buatkan paket penugasan siswa terstruktur untuk mata pelajaran: "{subject_name}", topik: "{topic}", jenjang: "{grade_level}", tingkat kesulitan: "{difficulty}" (ID Batch: {now_ts}).

{subject_guideline}

{format_instruction}

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Tugas Siswa: {topic}",
  "instructions": "Petunjuk pengerjaan penugasan langkah demi langkah...",
  "assignment_type": "{assignment_type_val}",
  "rubric": "Rubrik Penilaian: Skor 100 dengan kriteria...",
  "tasks": [
    "Tugas 1: Butir tugas operasional terukur...",
    "Tugas 2: Butir tugas pemecahan masalah / aplikasi nyata..."
  ],
  "questions": [
    {{
      "id": "q1",
      "question_text": "Teks soal pilihan ganda nomor 1...",
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
      "question_text": "Teks soal uraian / essay analitis...",
      "question_type": "ESSAY",
      "points": 20,
      "choices": [],
      "explanation": "Rubrik pedoman penskoran essay..."
    }}
  ]
}}

PENTING:
- Pastikan teks soal dan pilihan jawaban SESUAI ATURAN MAPEL ({subject_name}).
- Hasilkan TEPAT {total_questions} butir soal pada array 'questions' ({num_mcq} PG dan {num_essay} Essay).
- Nilai 'rubric' dan 'instructions' HARUS berupa STRING TEKS tunggal (BUKAN JSON object/map).
- Output HANYA JSON tanpa teks lain.
- Pastikan seluruh string multi-baris menggunakan escape sequence \n dan BUKAN karakter raw newline."#,
        assignment_type_val = if is_homework {
            "HOMEWORK_PR"
        } else {
            "STRUCTURED_QUESTIONS"
        }
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an educational assessor creating student assignments strictly adhering to Indonesian Kurikulum Merdeka in strict JSON.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.65, 3800).await?;
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

    let parsed: RawAssignmentResponse = serde_json::from_str(&sanitized).map_err(|e| {
        ApplicationError::Domain(DomainError::Validation(format!(
            "Invalid AI Assignment JSON: {e}"
        )))
    })?;

    let safe_visual_url = if include_images {
        Some(get_safe_curated_educational_image(subject_name, topic))
    } else {
        None
    };

    let title_str = parsed
        .title
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Tugas Siswa: {topic}"));
    let instructions_str = parsed
        .instructions
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Kerjakan penugasan berikut dengan seksama dan teliti.".to_string());
    let rubric_str = parsed.rubric.map(|v| json_value_to_string(&v)).filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Rubrik Penilaian: Skala Skor 0 - 100 berdasarkan ketepatan konsep dan kelengkapan analisis.".to_string());

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
        let q_text = q_val
            .get("question_text")
            .or_else(|| q_val.get("text"))
            .map(json_value_to_string)
            .unwrap_or_default();
        if q_text.trim().is_empty() {
            continue;
        }

        let mut choices: Vec<CbtQuestionChoiceDto> = Vec::new();
        if let Some(choices_val) = q_val.get("choices") {
            if let Some(c_arr) = choices_val.as_array() {
                for (c_idx, c_item) in c_arr.iter().enumerate() {
                    let c_text = c_item
                        .get("choice_text")
                        .or_else(|| c_item.get("text"))
                        .map(json_value_to_string)
                        .unwrap_or_else(|| json_value_to_string(c_item));
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
                    let c_text = format!("{k}. {}", json_value_to_string(v));
                    let is_correct = k.trim().eq_ignore_ascii_case(correct_key.trim());
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: c_text,
                        is_correct,
                    });
                }
            }
        }

        let explicit_type = q_val.get("question_type").and_then(|v| v.as_str());
        let question_type = explicit_type.map(|t| t.to_uppercase()).unwrap_or_else(|| {
            if choices.is_empty() {
                "ESSAY".to_string()
            } else {
                "MULTIPLE_CHOICE".to_string()
            }
        });

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
        let img = if include_images && idx < 2 {
            safe_visual_url.clone()
        } else {
            None
        };

        questions.push(AssignmentQuestionDto {
            id: q_id,
            question_text: q_text,
            question_type,
            points,
            image_url: img,
            choices,
            explanation,
        });
    }

    // Fallback if questions array was empty but tasks existed
    if questions.is_empty() && !tasks_vec.is_empty() {
        for (idx, task_str) in tasks_vec.iter().enumerate() {
            let img = if include_images && idx < 2 {
                safe_visual_url.clone()
            } else {
                None
            };
            questions.push(AssignmentQuestionDto {
                id: format!("asg-task-{}", idx + 1),
                question_text: task_str.clone(),
                question_type: "ESSAY".to_string(),
                points: 20,
                image_url: img,
                choices: Vec::new(),
                explanation: rubric_str.clone(),
            });
        }
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

// ─────────────────────────────────────────────────────────────────────────────
// 4. CBT QUIZ / EXAM GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CbtQuestionDto {
    pub id: String,
    pub question_text: String,
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
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    num_questions: usize,
    difficulty: &str,
) -> Result<GeneratedCbtQuizDto, ApplicationError> {
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
    let now_ts = chrono::Utc::now().timestamp_millis();

    let prompt = format!(
        r#"Anda adalah tim pembuat bank soal ujian CBT resmi sekolah (Kurikulum Merdeka).
Buatkan TEPAT {num_questions} butir soal pilihan ganda (opsi A-D) yang BERBEDA, DINAMIS, dan BERVARIASI untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}" tingkat kesulitan: {difficulty} (ID Batch: {now_ts}).

{subject_guideline}

PENTING & WAJIB:
- Array 'questions' HARUS BERISI TEPAT {num_questions} BUTIR SOAL LENGKAP (DILARANG KERAS HANYA MEMBUAT 1 SOAL!).
- Setiap soal harus unik, variatif, dan menguji aspek pemahaman yang berbeda dari materi {topic}.
- Kunci jawaban benar (correct_key) harus didistribusikan secara acak dan seimbang antara 'A', 'B', 'C', dan 'D' (jangan condong ke satu huruf saja).
- Pilihan A, B, C, D harus masuk akal, memiliki distraktor ilmiah yang baik, dan tidak ada pilihan konyol.

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Paket Soal CBT: {topic}",
  "description": "Ujian pilihan ganda {num_questions} butir soal materi {topic} ({subject_name}).",
  "questions": [
    {{
      "question_text": "Teks butir soal nomor 1 yang jelas dan kontekstual...",
      "options": {{
        "A": "Pilihan A",
        "B": "Pilihan B",
        "C": "Pilihan C",
        "D": "Pilihan D"
      }},
      "correct_key": "B",
      "explanation": "Pembahasan lengkap kenapa B benar...",
      "bloom_level": "C3",
      "points": 20
    }},
    {{
      "question_text": "Teks butir soal nomor 2 yang unik dan berbeda konteks...",
      "options": {{
        "A": "Pilihan A",
        "B": "Pilihan B",
        "C": "Pilihan C",
        "D": "Pilihan D"
      }},
      "correct_key": "C",
      "explanation": "Pembahasan lengkap kenapa C benar...",
      "bloom_level": "C4",
      "points": 20
    }}
  ]
}}

Output HANYA JSON tanpa teks pembuka atau penutup."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are a professional CBT exam author producing valid JSON question banks strictly respecting the subject matter rules and producing the exact requested count of distinct questions.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.65, 3800).await?;
    let cleaned = extract_clean_json(&raw);
    let sanitized = sanitize_json_control_chars(cleaned);

    #[derive(Deserialize)]
    struct RawQuizResponse {
        title: Option<serde_json::Value>,
        description: Option<serde_json::Value>,
        #[serde(default)]
        questions: Vec<serde_json::Value>,
    }

    let parsed: RawQuizResponse = serde_json::from_str(&sanitized).map_err(|e| {
        ApplicationError::Domain(DomainError::Validation(format!(
            "Invalid AI CBT Quiz JSON: {e}"
        )))
    })?;

    let total_q = parsed.questions.len().max(1) as i32;
    let default_pts = (100 / total_q).max(1);

    let mut questions: Vec<CbtQuestionDto> = Vec::new();

    for (idx, q_val) in parsed.questions.into_iter().enumerate() {
        let q_text = q_val
            .get("question_text")
            .or_else(|| q_val.get("text"))
            .map(json_value_to_string)
            .unwrap_or_default();
        if q_text.trim().is_empty() {
            continue;
        }

        let correct_key = q_val
            .get("correct_key")
            .and_then(|k| k.as_str())
            .unwrap_or("A")
            .trim()
            .to_uppercase();
        let mut choices: Vec<CbtQuestionChoiceDto> = Vec::new();

        if let Some(options_val) = q_val.get("options") {
            if let Some(map) = options_val.as_object() {
                for (key, val) in map {
                    let k_upper = key.trim().to_uppercase();
                    let is_correct = k_upper == correct_key;
                    choices.push(CbtQuestionChoiceDto {
                        choice_text: format!("{key}. {}", json_value_to_string(val)),
                        is_correct,
                    });
                }
            }
        } else if let Some(choices_val) = q_val.get("choices") {
            if let Some(arr) = choices_val.as_array() {
                for (c_idx, c_item) in arr.iter().enumerate() {
                    let c_text = c_item
                        .get("choice_text")
                        .or_else(|| c_item.get("text"))
                        .map(json_value_to_string)
                        .unwrap_or_else(|| json_value_to_string(c_item));
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
        }

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
            .unwrap_or(default_pts);

        questions.push(CbtQuestionDto {
            id: format!("gen-q-{}-{}", idx + 1, now_ts),
            question_text: q_text,
            choices,
            correct_key,
            explanation,
            bloom_level,
            points,
        });
    }

    let title_str = parsed
        .title
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Paket Soal CBT: {topic}"));
    let desc_str = parsed
        .description
        .map(|v| json_value_to_string(&v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!("Ujian CBT {num_questions} butir soal materi {topic} ({subject_name}).")
        });

    Ok(GeneratedCbtQuizDto {
        title: title_str,
        description: desc_str,
        questions,
    })
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
