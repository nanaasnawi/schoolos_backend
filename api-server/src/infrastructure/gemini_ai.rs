use reqwest::Client;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";
const DEFAULT_GEMINI_MODEL: &str = "gemini-3.1-flash-lite-image";
const STANDARD_GEMINI_MODELS_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const VERTEX_EXPRESS_MODELS_BASE: &str = "https://aiplatform.googleapis.com/v1/publishers/google/models";

static SHARED_GEMINI_CLIENT: OnceLock<Client> = OnceLock::new();

fn get_gemini_client() -> &'static Client {
    SHARED_GEMINI_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(45))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

fn get_gemini_config() -> Result<(String, String, String), String> {
    // Strictly load Gemini API key from Railway/environment variable (never hardcode secrets!)
    let api_key = std::env::var("GEMINI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "Variable GEMINI_API_KEY belum terpasang di Railway".to_string())?;

    let api_url = std::env::var("GEMINI_API_URL")
        .ok()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_GEMINI_API_URL.to_string());

    let model = std::env::var("GEMINI_MODEL")
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_GEMINI_MODEL.to_string());

    Ok((api_key, api_url, model))
}

#[derive(Clone, Copy, Debug)]
pub enum GeminiRouteKind {
    GoogleGenerateContent,
    VertexExpressGenerateContent,
    GoogleInteractions,
}

#[derive(Clone, Debug)]
pub struct ResolvedGeminiRoute {
    pub kind: GeminiRouteKind,
    pub model: String,
}

/// Zero-dependency standard + URL-safe Base64 decoder so Cargo.lock stays unchanged.
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let trimmed = input.trim();
    let raw = match trimmed.find(";base64,") {
        Some(idx) => &trimmed[idx + 8..],
        None => trimmed,
    };

    let mut out = Vec::with_capacity((raw.len() * 3) / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;

    for b in raw.bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn extract_image_from_gemini_json(val: &Value) -> Option<(String, Vec<u8>)> {
    if let Some(outputs) = val.get("outputs").and_then(|v| v.as_array()) {
        for item in outputs {
            if let Some(found) = extract_image_from_output_obj(item) {
                return Some(found);
            }
        }
    }

    if let Some(steps) = val.get("steps").and_then(|v| v.as_array()) {
        for step in steps {
            if let Some(outputs) = step.get("outputs").and_then(|v| v.as_array()) {
                for item in outputs {
                    if let Some(found) = extract_image_from_output_obj(item) {
                        return Some(found);
                    }
                }
            }
        }
    }

    if let Some(candidates) = val.get("candidates").and_then(|v| v.as_array()) {
        for cand in candidates {
            if let Some(parts) = cand
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    let inline = part.get("inlineData").or_else(|| part.get("inline_data"));
                    if let Some(in_obj) = inline {
                        let b64 = in_obj.get("data").and_then(|d| d.as_str()).unwrap_or("");
                        let mime = in_obj
                            .get("mimeType")
                            .or_else(|| in_obj.get("mime_type"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("image/png");
                        if let Some(bytes) = decode_base64(b64) {
                            return Some((mime.to_string(), bytes));
                        }
                    }
                }
            }
        }
    }

    None
}

fn extract_image_from_output_obj(item: &Value) -> Option<(String, Vec<u8>)> {
    let typ = item
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_lowercase();

    if typ == "image" || item.get("image_bytes").is_some() || item.get("mime_type").is_some() {
        let b64 = item
            .get("data")
            .and_then(|d| d.as_str())
            .or_else(|| item.get("image_bytes").and_then(|d| d.as_str()))
            .or_else(|| item.get("bytes").and_then(|d| d.as_str()))
            .or_else(|| {
                item.get("image")
                    .and_then(|img| img.get("data"))
                    .and_then(|d| d.as_str())
            })
            .unwrap_or("");

        if !b64.is_empty() {
            let mime = item
                .get("mime_type")
                .or_else(|| item.get("mimeType"))
                .and_then(|m| m.as_str())
                .unwrap_or("image/png");
            if let Some(bytes) = decode_base64(b64) {
                return Some((mime.to_string(), bytes));
            }
        }
    }

    None
}

fn build_infographic_visual_prompt(
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
) -> String {
    format!(
        "Create a vibrant, child-safe educational illustration for an Indonesian school infographic module.\n\
        Subject: {subject_name} | Student Level: {grade_level}\n\
        Visual Scene / Concept: {concept_prompt}\n\
        Art Direction: clean stylized 3D Pixar-inspired textbook illustration, crisp bright lighting, high visual clarity, clear subject focus, NO garbled or random text labels, NO watermarks."
    )
}

fn extract_google_error_message(txt: &str, status_code: u16) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(txt) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            let short = msg.chars().take(140).collect::<String>();
            return format!("HTTP {status_code}: {short}");
        }
    }
    let fallback = txt.trim().chars().take(120).collect::<String>();
    if fallback.is_empty() {
        format!("HTTP {status_code}")
    } else {
        format!("HTTP {status_code}: {fallback}")
    }
}

async fn call_generate_content_url(
    client: &Client,
    api_key: &str,
    target_url: &str,
    model: &str,
    prompt: &str,
    aspect_ratio: &str,
) -> Result<(String, Vec<u8>), String> {
    let payload_with_cfg = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"],
            "imageConfig": {
                "aspectRatio": aspect_ratio
            }
        }
    });

    let resp = client
        .post(target_url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload_with_cfg)
        .send()
        .await
        .map_err(|e| format!("Network error ({model}): {e}"))?;

    let status = resp.status();
    let txt = resp.text().await.unwrap_or_default();
    if status.is_success() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
            if let Some(img) = extract_image_from_gemini_json(&parsed) {
                return Ok(img);
            }
        }
        return Err(format!("{model}: respon 200 tetapi tidak berisi byte gambar"));
    }

    let first_err = extract_google_error_message(&txt, status.as_u16());

    if status.as_u16() == 400 {
        let payload_min = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "responseModalities": ["IMAGE"]
            }
        });
        if let Ok(resp2) = client
            .post(target_url)
            .header("x-goog-api-key", api_key)
            .header("Content-Type", "application/json")
            .json(&payload_min)
            .send()
            .await
        {
            let st2 = resp2.status();
            let txt2 = resp2.text().await.unwrap_or_default();
            if st2.is_success() {
                if let Ok(parsed2) = serde_json::from_str::<Value>(&txt2) {
                    if let Some(img) = extract_image_from_gemini_json(&parsed2) {
                        return Ok(img);
                    }
                }
            }
        }
    }

    Err(format!("{model} -> {first_err}"))
}

async fn call_interactions_url(
    client: &Client,
    api_key: &str,
    interactions_url: &str,
    model: &str,
    prompt: &str,
) -> Result<(String, Vec<u8>), String> {
    let payload = json!({
        "model": model,
        "input": prompt,
        "response_modalities": ["image"]
    });

    let resp = client
        .post(interactions_url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network error ({model}): {e}"))?;

    let status = resp.status();
    let txt = resp.text().await.unwrap_or_default();
    if status.is_success() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
            if let Some(img) = extract_image_from_gemini_json(&parsed) {
                return Ok(img);
            }
        }
        return Err(format!("{model} (interactions): respon 200 tanpa gambar"));
    }

    Err(format!(
        "{model} (interactions) -> {}",
        extract_google_error_message(&txt, status.as_u16())
    ))
}

async fn execute_single_route(
    client: &Client,
    api_key: &str,
    api_url: &str,
    route: &ResolvedGeminiRoute,
    prompt: &str,
    aspect_ratio: &str,
) -> Result<(String, Vec<u8>), String> {
    let model = route.model.as_str();
    match route.kind {
        GeminiRouteKind::GoogleGenerateContent => {
            let target_url = if !api_url.contains("aiplatform")
                && !api_url.contains("/interactions")
                && api_url.contains(":generateContent")
            {
                api_url.to_string()
            } else if !api_url.contains("aiplatform")
                && !api_url.contains("/interactions")
                && api_url.contains("/models")
            {
                format!("{}/{}:generateContent", api_url.trim_end_matches('/'), model)
            } else {
                format!("{}/{}:generateContent", STANDARD_GEMINI_MODELS_BASE, model)
            };
            call_generate_content_url(client, api_key, &target_url, model, prompt, aspect_ratio).await
        }
        GeminiRouteKind::VertexExpressGenerateContent => {
            let target_url = format!("{}/{}:generateContent", VERTEX_EXPRESS_MODELS_BASE, model);
            call_generate_content_url(client, api_key, &target_url, model, prompt, aspect_ratio).await
        }
        GeminiRouteKind::GoogleInteractions => {
            call_interactions_url(client, api_key, api_url, model, prompt).await
        }
    }
}

async fn persist_image(pool: &PgPool, mime: &str, bytes: Vec<u8>) -> String {
    let (clean_mime, ext) = if mime.contains("png") {
        ("image/png", "png")
    } else if mime.contains("webp") {
        ("image/webp", "webp")
    } else {
        ("image/jpeg", "jpg")
    };

    let filename = format!("ai_infografis_{}.{}", Uuid::new_v4(), ext);

    let db_res = sqlx::query(
        r#"
        INSERT INTO uploaded_files (filename, mime_type, file_data)
        VALUES ($1, $2, $3)
        ON CONFLICT (filename) DO UPDATE
        SET file_data = EXCLUDED.file_data, mime_type = EXCLUDED.mime_type, created_at = NOW()
        "#,
    )
    .bind(&filename)
    .bind(clean_mime)
    .bind(bytes.as_slice())
    .execute(pool)
    .await;

    if let Err(e) = db_res {
        tracing::warn!("Failed to persist Gemini infographic image to uploaded_files: {e}");
    }

    let _ = tokio::fs::create_dir_all("uploads").await;
    let disk_path = std::path::PathBuf::from("uploads").join(&filename);
    let _ = tokio::fs::write(&disk_path, &bytes).await;

    format!("/uploads/{filename}")
}

/// Generates the Hero cover image while probing across Google AI Studio, Vertex AI Express
/// (for `AQ.` express keys), and Interactions endpoints, returning both the `/uploads/...` URL
/// and the winning route so subsequent cards run concurrently without redundant retries.
pub async fn probe_and_generate_hero_image(
    pool: &PgPool,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
    aspect_ratio: &str,
) -> Result<(String, ResolvedGeminiRoute), String> {
    let (api_key, api_url, configured_model) = get_gemini_config()?;
    let client = get_gemini_client();
    let prompt = build_infographic_visual_prompt(concept_prompt, subject_name, grade_level);

    let mut models = vec![configured_model.clone()];
    for fallback in [
        "gemini-3.1-flash-lite-image",
        "gemini-3.1-flash-image",
        "gemini-3.1-flash-image-preview",
        "gemini-2.5-flash-image",
    ] {
        if !models.iter().any(|m| m == fallback) {
            models.push(fallback.to_string());
        }
    }

    let is_vertex_url = api_url.to_lowercase().contains("aiplatform.googleapis.com");
    let is_interactions_url = api_url.to_lowercase().contains("/interactions");

    let kinds: Vec<GeminiRouteKind> = if is_vertex_url {
        vec![
            GeminiRouteKind::VertexExpressGenerateContent,
            GeminiRouteKind::GoogleGenerateContent,
        ]
    } else if api_key.starts_with("AQ.") {
        // An AQ. key may be either Google AI Studio or Vertex AI Express Mode
        if is_interactions_url {
            vec![
                GeminiRouteKind::GoogleGenerateContent,
                GeminiRouteKind::VertexExpressGenerateContent,
                GeminiRouteKind::GoogleInteractions,
            ]
        } else {
            vec![
                GeminiRouteKind::GoogleGenerateContent,
                GeminiRouteKind::VertexExpressGenerateContent,
            ]
        }
    } else if is_interactions_url {
        vec![
            GeminiRouteKind::GoogleGenerateContent,
            GeminiRouteKind::GoogleInteractions,
        ]
    } else {
        vec![GeminiRouteKind::GoogleGenerateContent]
    };

    let mut last_err = "Gemini gagal menghasilkan gambar".to_string();

    for kind in &kinds {
        for model in &models {
            if matches!(kind, GeminiRouteKind::GoogleInteractions) && model.contains("lite") {
                continue;
            }
            let route = ResolvedGeminiRoute {
                kind: *kind,
                model: model.clone(),
            };
            match execute_single_route(
                client,
                api_key.as_str(),
                api_url.as_str(),
                &route,
                &prompt,
                aspect_ratio,
            )
            .await
            {
                Ok((mime, bytes)) => {
                    tracing::info!("Gemini hero image succeeded via {:?} ({})", route.kind, route.model);
                    let url = persist_image(pool, &mime, bytes).await;
                    return Ok((url, route));
                }
                Err(err) => {
                    tracing::warn!("Gemini probe failed ({:?}): {}", route.kind, err);
                    last_err = err;
                }
            }
        }
    }

    Err(last_err)
}

/// Fast direct generation for infographic step cards using the already-proven winning route.
pub async fn generate_card_image_with_route(
    pool: &PgPool,
    route: &ResolvedGeminiRoute,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
    aspect_ratio: &str,
) -> Option<String> {
    let (api_key, api_url, _) = get_gemini_config().ok()?;
    let client = get_gemini_client();
    let prompt = build_infographic_visual_prompt(concept_prompt, subject_name, grade_level);

    match execute_single_route(client, api_key.as_str(), api_url.as_str(), route, &prompt, aspect_ratio).await {
        Ok((mime, bytes)) => Some(persist_image(pool, &mime, bytes).await),
        Err(err) => {
            tracing::warn!("Gemini card image failed ({}): {}", route.model, err);
            None
        }
    }
}
