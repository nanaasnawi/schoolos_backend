use std::time::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use school_core::common::error::{ApplicationError, DomainError};

const DEFAULT_NVIDIA_MODEL: &str = "nvidia/ising-calibration-1.5-31b";
const DEFAULT_NVIDIA_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
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
    let key = env_key
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| {
            // Fallback aman jika environment variable belum dikonfigurasi di cloud dashboard
            "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".to_string()
        });

    let model = std::env::var("NVIDIA_MODEL").unwrap_or_else(|_| DEFAULT_NVIDIA_MODEL.to_string());
    let url = std::env::var("NVIDIA_API_URL").unwrap_or_else(|_| DEFAULT_NVIDIA_URL.to_string());
    Ok((key, model, url))
}

/// Call NVIDIA NIM API
pub async fn call_nvidia_nim(
    messages: Vec<ChatMessage<'_>>,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, ApplicationError> {
    let (api_key, model, api_url) = get_config()?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| ApplicationError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let payload = ChatCompletionRequest {
        model: &model,
        messages,
        temperature,
        max_tokens,
    };

    let resp = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApplicationError::Internal(format!("Failed to send request to NVIDIA NIM: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(ApplicationError::Internal(format!(
            "NVIDIA NIM API responded with HTTP {status}: {err_body}"
        )));
    }

    let parsed: ChatCompletionResponse = resp
        .json()
        .await
        .map_err(|e| ApplicationError::Internal(format!("Failed to parse NVIDIA NIM response: {e}")))?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| ApplicationError::Internal("NVIDIA NIM returned empty choices content".to_string()))?;

    Ok(content)
}

fn extract_clean_json(text: &str) -> &str {
    let mut trimmed = text.trim();
    if trimmed.starts_with("```json") {
        trimmed = trimmed.trim_start_matches("```json").trim();
    } else if trimmed.starts_with("```") {
        trimmed = trimmed.trim_start_matches("```").trim();
    }
    if trimmed.ends_with("```") {
        trimmed = trimmed.trim_end_matches("```").trim();
    }

    if let (Some(first), Some(last)) = (trimmed.find(|c| c == '{' || c == '['), trimmed.rfind(|c| c == '}' || c == ']')) {
        if last >= first {
            return &trimmed[first..=last];
        }
    }
    trimmed
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

#[derive(Deserialize)]
struct RawInfographicCard {
    step_number: u32,
    headline: String,
    summary: String,
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
    let prompt = format!(
        r#"Anda adalah pendidik spesialis penyusun media pembelajaran visual Kurikulum Merdeka di Indonesia.
Buatkan materi INFOGRAFIS INTERAKTIF untuk siswa jenjang {grade_level} pada mata pelajaran {subject_name} dengan topik: "{topic}".

Format output HARUS berupa JSON murni dengan skema persis:
{{
  "title": "Judul Infografis Menarik",
  "description": "Deskripsi singkat modul 1-2 kalimat.",
  "cards": [
    {{
      "step_number": 1,
      "headline": "Judul Poin Visual 1",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mudah dipahami anak.",
      "image_suggestion": "Deskripsi ide ilustrasi atau diagram yang cocok"
    }},
    {{
      "step_number": 2,
      "headline": "Judul Poin Visual 2",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mudah dipahami anak.",
      "image_suggestion": "Deskripsi ide ilustrasi atau diagram yang cocok"
    }},
    {{
      "step_number": 3,
      "headline": "Judul Poin Visual 3",
      "summary": "Penjelasan ringkas 2-3 kalimat yang mudah dipahami anak.",
      "image_suggestion": "Deskripsi ide ilustrasi atau diagram yang cocok"
    }},
    {{
      "step_number": 4,
      "headline": "Kesimpulan & Tahukah Kamu?",
      "summary": "Fakta menarik dan intisari penting untuk diingat siswa.",
      "image_suggestion": "Deskripsi ide ilustrasi atau diagram yang cocok"
    }}
  ]
}}

Output HANYA JSON tanpa teks pembuka/penutup."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an educational designer creating structured infographic cards in strict JSON.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.3, 1800).await?;
    let cleaned = extract_clean_json(&raw);

    let parsed: RawInfographicResponse = serde_json::from_str(cleaned)
        .map_err(|e| ApplicationError::Domain(DomainError::Validation(format!("Invalid AI Infographic JSON: {e}"))))?;

    let now_ts = chrono::Utc::now().timestamp_millis();
    let mut blocks = Vec::new();

    // 1. Hero Cover Image for the Magazine / Infographic Header
    let topic_slug: String = topic
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => ' ',
        })
        .collect();
    let topic_encoded = topic_slug.split_whitespace().collect::<Vec<_>>().join("%20");
    let cover_url = format!(
        "https://image.pollinations.ai/prompt/{topic_encoded}%20educational%20infographic%20magazine%20cover%20vibrant%20clean%20aesthetic?width=1200&height=630&nologo=true"
    );
    blocks.push(InfographicBlockDto {
        id: format!("ai-block-hero-cover-{}", now_ts),
        block_type: "IMAGE".to_string(),
        content: cover_url,
    });

    for (idx, card) in parsed.cards.into_iter().enumerate() {
        if let Some(img_desc) = card.image_suggestion {
            if !img_desc.trim().is_empty() {
                let img_slug: String = img_desc
                    .chars()
                    .map(|c| match c {
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
                        _ => ' ',
                    })
                    .collect();
                let img_encoded = img_slug.split_whitespace().collect::<Vec<_>>().join("%20");
                let image_url = format!(
                    "https://image.pollinations.ai/prompt/{}%20educational%20magazine%20illustration%20vibrant%20clean?width=1000&height=600&nologo=true",
                    img_encoded
                );
                blocks.push(InfographicBlockDto {
                    id: format!("ai-block-img-{}-{}", idx + 1, now_ts),
                    block_type: "IMAGE".to_string(),
                    content: image_url,
                });
            }
        }

        blocks.push(InfographicBlockDto {
            id: format!("ai-block-text-{}-{}", idx + 1, now_ts),
            block_type: "TEXT".to_string(),
            content: format!("### 📌 {}. {}\n\n{}", card.step_number, card.headline, card.summary),
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

fn parse_or_extract_article(raw: &str, default_topic: &str, default_subject: &str) -> GeneratedArticleDto {
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
                if end > 0 { &after_quote[..end] } else { after_quote }
            } else {
                after_quote
            };
            content_str.replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\")
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
    let prompt = format!(
        r#"Anda adalah guru ahli Kurikulum Merdeka di Indonesia.
Tuliskan naskah ARTIKEL PEMBELAJARAN lengkap untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}".

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Judul Naskah Pembelajaran",
  "description": "Deskripsi pengantar singkat 1-2 kalimat.",
  "markdown_article": "Isi lengkap artikel berformat Markdown terstruktur:\n# Judul\n## 1. Pengantar & Apersepsi Menarik\n## 2. Pembahasan Konsep Inti\n## 3. Contoh Nyata & Aplikasi\n## 4. Rangkuman Intisari\n## 5. Glosarium / Kamus Kata Sulit"
}}

Pastikan teks padat, jelas, format JSON valid dengan escape karakter benar, dan selalu ditutup sempurna. Output HANYA JSON tanpa teks lain."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an expert Indonesian teacher creating educational articles in strict JSON.",
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
// 3. ASSIGNMENT GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedAssignmentDto {
    pub title: String,
    pub instructions: String,
    pub rubric: String,
    pub tasks: Vec<String>,
}

pub async fn generate_assignment(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
) -> Result<GeneratedAssignmentDto, ApplicationError> {
    let prompt = format!(
        r#"Anda adalah guru pengembang tugas siswa Kurikulum Merdeka.
Buatkan LEMBAR PENUGASAN SISWA untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}".

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Judul Tugas Siswa",
  "instructions": "Petunjuk pengerjaan langkah demi langkah bagi siswa.",
  "tasks": [
    "Tugas 1: ...",
    "Tugas 2: ...",
    "Tugas 3: ..."
  ],
  "rubric": "Rubrik penilaian objektif (Skala 1-100 dengan indikator Sangat Baik, Baik, Cukup)."
}}

Output HANYA JSON tanpa teks lain."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are an educational assessor creating student assignments in strict JSON.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.3, 1800).await?;
    let cleaned = extract_clean_json(&raw);

    let parsed: GeneratedAssignmentDto = serde_json::from_str(cleaned)
        .map_err(|e| ApplicationError::Domain(DomainError::Validation(format!("Invalid AI Assignment JSON: {e}"))))?;

    Ok(parsed)
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. CBT QUIZ / EXAM GENERATION DTOs & FUNCTION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CbtQuestionChoiceDto {
    pub choice_text: String,
    pub is_correct: bool,
}

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

#[derive(Deserialize)]
struct RawQuizQuestion {
    question_text: String,
    options: std::collections::BTreeMap<String, String>,
    correct_key: String,
    explanation: Option<String>,
    bloom_level: Option<String>,
    points: Option<i32>,
}

#[derive(Deserialize)]
struct RawQuizResponse {
    title: String,
    description: String,
    questions: Vec<RawQuizQuestion>,
}

pub async fn generate_cbt_quiz(
    topic: &str,
    grade_level: &str,
    subject_name: &str,
    num_questions: usize,
    difficulty: &str,
) -> Result<GeneratedCbtQuizDto, ApplicationError> {
    let prompt = format!(
        r#"Anda adalah tim pembuat soal ujian CBT resmi sekolah (Kurikulum Merdeka).
Buatkan {num_questions} butir soal pilihan ganda (opsi A-D) untuk jenjang {grade_level} mata pelajaran {subject_name} dengan topik: "{topic}" tingkat kesulitan: {difficulty}.

Format output HARUS berupa JSON murni dengan skema:
{{
  "title": "Paket Soal CBT: {topic}",
  "description": "Ujian pilihan ganda {num_questions} butir soal materi {topic}.",
  "questions": [
    {{
      "question_text": "Teks soal yang jelas...",
      "options": {{
        "A": "Pilihan A",
        "B": "Pilihan B",
        "C": "Pilihan C",
        "D": "Pilihan D"
      }},
      "correct_key": "B",
      "explanation": "Pembahasan lengkap kenapa jawaban B benar.",
      "bloom_level": "C3",
      "points": 20
    }}
  ]
}}

Output HANYA JSON tanpa teks lain."#
    );

    let messages = vec![
        ChatMessage {
            role: "system",
            content: "You are a professional CBT exam author producing valid JSON question banks.",
        },
        ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw = call_nvidia_nim(messages, 0.3, 2500).await?;
    let cleaned = extract_clean_json(&raw);

    let parsed: RawQuizResponse = serde_json::from_str(cleaned)
        .map_err(|e| ApplicationError::Domain(DomainError::Validation(format!("Invalid AI CBT Quiz JSON: {e}"))))?;

    let now_ts = chrono::Utc::now().timestamp_millis();
    let total_q = parsed.questions.len().max(1) as i32;
    let default_pts = (100 / total_q).max(1);

    let questions = parsed
        .questions
        .into_iter()
        .enumerate()
        .map(|(idx, q)| {
            let correct_upper = q.correct_key.trim().to_uppercase();
            let choices = q
                .options
                .into_iter()
                .map(|(key, text)| {
                    let is_correct = key.trim().to_uppercase() == correct_upper;
                    CbtQuestionChoiceDto {
                        choice_text: format!("{key}. {text}"),
                        is_correct,
                    }
                })
                .collect();

            CbtQuestionDto {
                id: format!("gen-q-{}-{}", idx + 1, now_ts),
                question_text: q.question_text,
                choices,
                correct_key: correct_upper,
                explanation: q.explanation.unwrap_or_default(),
                bloom_level: q.bloom_level.unwrap_or_else(|| "C3".to_string()),
                points: q.points.unwrap_or(default_pts),
            }
        })
        .collect();

    Ok(GeneratedCbtQuizDto {
        title: parsed.title,
        description: parsed.description,
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

    let parsed: RawAnalytics = serde_json::from_str(cleaned)
        .map_err(|e| ApplicationError::Domain(DomainError::Validation(format!("Invalid AI Analytics JSON: {e}"))))?;

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
