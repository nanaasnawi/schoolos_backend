use school_core::common::error::{ApplicationError, DomainError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_NVIDIA_MODEL: &str = "nvidia/nemotron-3-ultra-550b-a55b";
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
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| ApplicationError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let mut candidate_models = vec![configured_model.as_str()];
    let active_models = [
        "nvidia/nemotron-3-ultra-550b-a55b",
        "nvidia/ising-calibration-1.5-31b",
        "stepfun-ai/step-5-preview",
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

    let raw = match call_nvidia_nim(messages, 0.3, 1800).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for infographic: {e}. Utilizing pedagogical fallback.");
            return Ok(fallback_generate_infographic(topic, grade_level, subject_name).await);
        }
    };
    let cleaned = extract_clean_json(&raw);

    let parsed: RawInfographicResponse = match serde_json::from_str(cleaned) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse AI Infographic JSON ({e}). Utilizing pedagogical fallback.");
            return Ok(fallback_generate_infographic(topic, grade_level, subject_name).await);
        }
    };

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

pub async fn fallback_generate_infographic(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> GeneratedInfographicDto {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let cover_url = search_real_educational_image(topic, subject_name).await;
    let mut blocks = Vec::new();

    blocks.push(InfographicBlockDto {
        id: format!("ai-block-hero-cover-{}", now_ts),
        block_type: "IMAGE".to_string(),
        content: cover_url,
    });

    let cards = [
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
    ];

    for (step_no, headline, summary) in cards {
        let img = get_safe_curated_educational_image(subject_name, topic);
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-img-{}-{}", step_no, now_ts),
            block_type: "IMAGE".to_string(),
            content: img,
        });
        blocks.push(InfographicBlockDto {
            id: format!("ai-block-text-{}-{}", step_no, now_ts),
            block_type: "TEXT".to_string(),
            content: format!("### 📌 {}. {}\n\n{}", step_no, headline, summary),
        });
    }

    GeneratedInfographicDto {
        title: format!("Infografis Modul: {topic}"),
        description: format!("Media pembelajaran visual Kurikulum Merdeka jenjang {grade_level} ({subject_name})."),
        blocks,
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

pub fn clean_choice_text(raw: &str) -> String {
    let trimmed = raw.trim();
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
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() <= 1 {
        return raw.trim().to_string();
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

    let slot_instructions = if is_homework {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {num_essay} BUTIR SOAL ESSAY / LEMBAR KERJA.\n- Butir 1 sampai {num_essay}: Tipe 'ESSAY' (analitis / tugas observasi kontekstual)."
        )
    } else {
        format!(
            "Array 'questions' WAJIB BERISI TEPAT {total_questions} BUTIR SOAL DENGAN PEMBAGIAN PERSIS:\n- Butir 1 sampai {num_mcq}: SOAL PILIHAN GANDA ('question_type': 'MULTIPLE_CHOICE') dengan 4 pilihan A, B, C, D dan kunci is_correct tepat 1 true.\n- Butir {essay_start} sampai {total_questions}: SOAL URAIAN / ESSAY ('question_type': 'ESSAY') dengan rubrik pedoman penskoran komprehensif.\nDILARANG KERAS mengurangi jumlah soal! Jumlah soal di array questions harus PERSIS {total_questions} butir.",
            essay_start = num_mcq + 1
        )
    };

    let prompt = format!(
        r#"Anda adalah pengembang asesmen resmi Kurikulum Merdeka Indonesia.
Buatkan paket penugasan siswa terstruktur untuk mata pelajaran: "{subject_name}", topik: "{topic}", jenjang: "{grade_level}", tingkat kesulitan: "{difficulty}" (Batch ID: {now_ts}).

{subject_guideline}

ATURAN STRUKTUR DAN JUMLAH SOAL:
{slot_instructions}

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
- Array 'questions' HARUS berisi TEPAT {total_questions} butir soal ({num_mcq} PG dan {num_essay} Essay). Jangan berhenti menulis sebelum mencapai {total_questions} butir!
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

    let raw = match call_nvidia_nim(messages, 0.65, 3800).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for assignment: {e}. Utilizing pedagogical fallback.");
            return Ok(fallback_generate_assignment(
                topic,
                grade_level,
                subject_name,
                assignment_format,
                num_mcq,
                num_essay,
                difficulty,
                include_images,
            ));
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
            return Ok(fallback_generate_assignment(
                topic,
                grade_level,
                subject_name,
                assignment_format,
                num_mcq,
                num_essay,
                difficulty,
                include_images,
            ));
        }
    };

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

    // ── Enforce Exact Counts for PG & Essay ──
    if !is_homework {
        let mut final_mcqs: Vec<AssignmentQuestionDto> = Vec::new();
        let mut final_essays: Vec<AssignmentQuestionDto> = Vec::new();

        for q in questions {
            if q.question_type == "MULTIPLE_CHOICE" && !q.choices.is_empty() {
                final_mcqs.push(q);
            } else {
                final_essays.push(q);
            }
        }

        // Pad missing MCQs if AI returned fewer than requested
        while final_mcqs.len() < num_mcq {
            let idx = final_mcqs.len() + 1;
            final_mcqs.push(AssignmentQuestionDto {
                id: format!("asg-mcq-pad-{idx}-{now_ts}"),
                question_text: format!("Pertanyaan pemahaman konsep {subject_name} butir ke-{idx} terkait materi {topic}:"),
                question_type: "MULTIPLE_CHOICE".to_string(),
                points: 10,
                image_url: None,
                choices: vec![
                    CbtQuestionChoiceDto {
                        choice_text: format!("A. Konsep esensial terapan {topic} secara tepat dan sahih"),
                        is_correct: true,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: "B. Pernyataan alternatif yang kurang spesifik".to_string(),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: "C. Distraktor teoritis sekunder".to_string(),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: "D. Opsi yang bertentangan dengan prinsip materi".to_string(),
                        is_correct: false,
                    },
                ],
                explanation: format!("Kunci A merupakan konsep mendasar pada materi {topic} ({subject_name})."),
            });
        }

        // Pad missing Essays if AI returned fewer than requested
        while final_essays.len() < num_essay {
            let idx = final_essays.len() + 1;
            final_essays.push(AssignmentQuestionDto {
                id: format!("asg-essay-pad-{idx}-{now_ts}"),
                question_text: format!("Jelaskan secara mendalam analisis dan penerapan konsep {topic} dalam konteks nyata (Aspek Evaluasi {idx}):"),
                question_type: "ESSAY".to_string(),
                points: 20,
                image_url: None,
                choices: Vec::new(),
                explanation: format!("Rubrik: Skor maksimal jika siswa mampu menjabarkan konsep {topic} disertai contoh konkret dan argumentasi logis yang runtut."),
            });
        }

        final_mcqs.truncate(num_mcq);
        final_essays.truncate(num_essay);

        questions = final_mcqs;
        questions.extend(final_essays);
    } else {
        while questions.len() < num_essay {
            let idx = questions.len() + 1;
            questions.push(AssignmentQuestionDto {
                id: format!("asg-hw-pad-{idx}-{now_ts}"),
                question_text: format!("Tugas Mandiri {idx}: Lakukan pengamatan dan susun laporan terstruktur mengenai {topic} sesuai panduan modul {subject_name}."),
                question_type: "ESSAY".to_string(),
                points: (100 / (num_essay.max(1) as i32)).max(10),
                image_url: None,
                choices: Vec::new(),
                explanation: format!("Rubrik: Penilaian berbasis kelengkapan laporan, orisinalitas analisis, dan ketepatan kaidah {subject_name}."),
            });
        }
        questions.truncate(num_essay);
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
            q_text = format!("(Butir {q_num}) {q_text}");
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
    include_images: bool,
) -> GeneratedAssignmentDto {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let is_homework = assignment_format
        .map(|f| f.to_uppercase() == "HOMEWORK_PR")
        .unwrap_or(false);

    let safe_image = if include_images {
        Some(get_safe_curated_educational_image(subject_name, topic))
    } else {
        None
    };

    let mcq_templates = generate_pedagogical_question_templates(subject_name, topic, grade_level, difficulty, num_mcq);
    let mut questions = Vec::new();

    if !is_homework {
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
                explanation: expl,
            });
        }
    }

    let essay_count = if is_homework { num_essay.max(1) } else { num_essay };
    let essay_pts = (100 / essay_count.max(1) as i32).max(15);

    let essay_prompts = [
        format!("Jelaskan pemahaman komprehensif Anda mengenai konsep '{topic}' pada mata pelajaran {subject_name} ({grade_level}). Tuliskan minimal dua contoh penerapan nyata dalam kehidupan sehari-hari!"),
        format!("Lakukan analisis terhadap sebuah permasalahan kontekstual yang berkaitan dengan materi '{topic}'. Uraikan langkah-langkah pemecahan masalah secara terstruktur dan berikan kesimpulan logis!"),
        format!("Susunlah sebuah telaah kritis mengenai tantangan yang dihadapi dalam penguasaan materi '{topic}', serta usulkan ide inovatif untuk mengatasinya!"),
        format!("Buatlah refleksi pembelajaran mendalam mengenai nilai-nilai baru yang Anda peroleh setelah mempelajari '{topic}', dan bagaimana materi ini membentuk cara berpikir kritis Anda!"),
    ];

    for idx in 0..essay_count {
        let prompt_text = essay_prompts[idx % essay_prompts.len()].clone();
        let q_num = questions.len() + 1;
        questions.push(AssignmentQuestionDto {
            id: format!("asg-essay-{}-{now_ts}", idx + 1),
            question_text: if essay_count > essay_prompts.len() {
                format!("(Uraian {q_num}) {prompt_text}")
            } else {
                prompt_text
            },
            question_type: "ESSAY".to_string(),
            points: essay_pts,
            image_url: None,
            choices: Vec::new(),
            explanation: "Rubrik Penskoran: Skor maksimal diberikan bila siswa menjabarkan konsep secara tepat, argumentasi logis runtut, disertai contoh kontekstual yang relevan (Skala 0 - 100).".to_string(),
        });
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
    quiz_format: Option<&str>,
    num_mcq: usize,
    num_essay: usize,
    difficulty: &str,
    _include_images: bool,
) -> Result<GeneratedCbtQuizDto, ApplicationError> {
    let subject_guideline = build_subject_pedagogical_instructions(subject_name, grade_level);
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

    let composition_instruction = if is_mixed {
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

    let prompt = format!(
        r#"Anda adalah tim pembuat bank soal ujian CBT resmi sekolah (Kurikulum Merdeka).
Buatkan TEPAT {total_questions} butir soal yang BERBEDA, DINAMIS, dan BERVARIASI untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}" tingkat kesulitan: {difficulty} (ID Batch: {now_ts}).

{subject_guideline}

{composition_instruction}

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

Output HANYA JSON tanpa teks pembuka atau penutup."#,
        sample_second_type = if is_mixed && num_essay > 0 {
            "ESSAY"
        } else {
            "MULTIPLE_CHOICE"
        },
        sample_second_pts = if is_mixed && num_essay > 0 { 20 } else { 10 }
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

    let raw = match call_nvidia_nim(messages, 0.65, 4500).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("call_nvidia_nim failed for CBT quiz: {e}. Utilizing pedagogical fallback.");
            return Ok(fallback_generate_cbt_quiz(
                topic,
                grade_level,
                subject_name,
                quiz_format,
                num_mcq,
                num_essay,
                difficulty,
                _include_images,
            ));
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
            return Ok(fallback_generate_cbt_quiz(
                topic,
                grade_level,
                subject_name,
                quiz_format,
                num_mcq,
                num_essay,
                difficulty,
                _include_images,
            ));
        }
    };

    let default_pts = (100 / total_questions.max(1) as i32).max(5);
    let mut questions: Vec<CbtQuestionDto> = Vec::new();

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

        questions.push(CbtQuestionDto {
            id: format!("gen-q-{}-{}", idx + 1, now_ts),
            question_text: q_text,
            question_type: Some(question_type),
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

        while final_mcqs.len() < num_mcq {
            let idx = final_mcqs.len() + 1;
            final_mcqs.push(CbtQuestionDto {
                id: format!("cbt-mcq-pad-{idx}-{now_ts}"),
                question_text: format!(
                    "Dalam konteks pembelajaran {}, manakah penjelasan yang paling tepat mengenai implikasi materi {}?",
                    subject_name, topic
                ),
                question_type: Some("MULTIPLE_CHOICE".to_string()),
                choices: vec![
                    CbtQuestionChoiceDto {
                        choice_text: format!("Menganalisis prinsip dan keterkaitan esensial materi {} secara sistematis", topic),
                        is_correct: true,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Memusatkan kajian hanya pada aspek teoritis tanpa implementasi"),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Mengabaikan variabel kontekstual dalam evaluasi materi"),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Menyamaratakan seluruh kondisi tanpa membedakan indikator utama"),
                        is_correct: false,
                    },
                ],
                correct_key: "A".to_string(),
                explanation: format!(
                    "Pemahaman mendalam tentang {} pada mata pelajaran {} memerlukan analisis sistematis terhadap seluruh prinsip dasarnya.",
                    topic, subject_name
                ),
                bloom_level: "C3".to_string(),
                points: default_pts,
            });
        }

        while final_essays.len() < num_essay {
            let idx = final_essays.len() + 1;
            final_essays.push(CbtQuestionDto {
                id: format!("cbt-essay-pad-{idx}-{now_ts}"),
                question_text: format!("Analisis dan uraikan pemahaman Anda mengenai penerapan materi {topic} dalam kehidupan sehari-hari (Soal Uraian {idx}):"),
                question_type: Some("ESSAY".to_string()),
                choices: Vec::new(),
                correct_key: String::new(),
                explanation: format!("Rubrik: Skor maksimal jika siswa mampu menjabarkan konsep {topic} secara sistematis dengan bukti/contoh logis."),
                bloom_level: "C4".to_string(),
                points: 20,
            });
        }

        final_mcqs.truncate(num_mcq);
        final_essays.truncate(num_essay);

        questions = final_mcqs;
        questions.extend(final_essays);
    } else {
        while questions.len() < total_questions {
            let idx = questions.len() + 1;
            questions.push(CbtQuestionDto {
                id: format!("cbt-mcq-pad-{idx}-{now_ts}"),
                question_text: format!("Pertanyaan pilihan ganda evaluasi materi {topic} butir ke-{idx} ({subject_name}):"),
                question_type: Some("MULTIPLE_CHOICE".to_string()),
                choices: vec![
                    CbtQuestionChoiceDto {
                        choice_text: format!("Prinsip dan pemahaman utama {} yang terverifikasi secara sahih", topic),
                        is_correct: true,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Kajian sekunder yang kurang relevan dengan pokok bahasan"),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Asumsi umum yang belum teruji keabsahannya"),
                        is_correct: false,
                    },
                    CbtQuestionChoiceDto {
                        choice_text: format!("Pernyataan yang berlawanan dengan kaidah {}", subject_name),
                        is_correct: false,
                    },
                ],
                correct_key: "A".to_string(),
                explanation: format!("Pilihan A merupakan kunci jawaban yang tepat sesuai kaidah {subject_name}."),
                bloom_level: "C3".to_string(),
                points: default_pts,
            });
        }
        questions.truncate(total_questions);
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
            format!("Ujian CBT {total_questions} butir soal materi {topic} ({subject_name}).")
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
            choices,
            correct_key: key_labels[correct_idx % 4].to_string(),
            explanation: expl,
            bloom_level: bloom_levels[idx % bloom_levels.len()].to_string(),
            points: default_pts,
        });
    }

    if is_mixed && num_essay > 0 {
        let essay_prompts = [
            format!("Analisis dan uraikan secara mendalam bagaimana prinsip utama '{topic}' ({subject_name}) diterapkan dalam memecahkan masalah nyata bagi siswa {grade_level}!"),
            format!("Berdasarkan materi '{topic}', jelaskan hubungan sebab-akibat antar-komponen utama dan berikan satu studi kasus konkret beserta pembahasannya!"),
            format!("Evaluasilah kelebihan dan tantangan dalam penerapan konsep '{topic}' serta rumuskan rekomendasi solusi yang inovatif dan terukur!"),
        ];

        for idx in 0..num_essay {
            let prompt = essay_prompts[idx % essay_prompts.len()].clone();
            let q_num = questions.len() + 1;
            questions.push(CbtQuestionDto {
                id: format!("cbt-essay-{}-{now_ts}", idx + 1),
                question_text: if num_essay > essay_prompts.len() {
                    format!("(Uraian {q_num}) {prompt}")
                } else {
                    prompt
                },
                question_type: Some("ESSAY".to_string()),
                choices: Vec::new(),
                correct_key: "ESSAY".to_string(),
                explanation: "Rubrik Penskoran: Skor penuh diberikan bila jawaban memuat analisis konsep akurat, argumentasi logis runtut, dan bukti/contoh aplikatif yang valid.".to_string(),
                bloom_level: "C4".to_string(),
                points: 20,
            });
        }
    }

    GeneratedCbtQuizDto {
        title: format!("Paket Soal CBT: {topic}"),
        description: format!("Ujian CBT {total_questions} butir soal materi {topic} ({subject_name}) jenjang {grade_level} ({difficulty})."),
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
