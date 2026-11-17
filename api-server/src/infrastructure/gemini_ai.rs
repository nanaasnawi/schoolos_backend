use reqwest::Client;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";
const DEFAULT_GEMINI_MODEL: &str = "gemini-3.1-flash-lite-image";
const STANDARD_GEMINI_MODELS_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

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

fn get_gemini_config() -> Option<(String, String, String)> {
    // Strictly load Gemini API key from Railway/environment variable (never hardcode secrets!)
    let api_key = std::env::var("GEMINI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())?;

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

    Some((api_key, api_url, model))
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
    // 1. Gemini Interactions API: `outputs` top-level array
    if let Some(outputs) = val.get("outputs").and_then(|v| v.as_array()) {
        for item in outputs {
            if let Some(found) = extract_image_from_output_obj(item) {
                return Some(found);
            }
        }
    }

    // 2. Gemini Interactions API: `steps[].outputs[]`
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

    // 3. Standard Gemini generateContent: `candidates[].content.parts[].inlineData` / `inline_data`
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

async fn call_gemini_generate_content(
    client: &Client,
    api_key: &str,
    target_url: &str,
    model: &str,
    prompt: &str,
    aspect_ratio: &str,
) -> Option<(String, Vec<u8>)> {
    let payload_with_cfg = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"],
            "imageConfig": {
                "aspectRatio": aspect_ratio,
                "imageSize": "1K"
            }
        }
    });

    if let Ok(resp) = client
        .post(target_url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload_with_cfg)
        .send()
        .await
    {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        if status.is_success() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                if let Some(img) = extract_image_from_gemini_json(&parsed) {
                    tracing::info!("Gemini generateContent ({model}) produced infographic image");
                    return Some(img);
                }
            }
            tracing::warn!(
                "Gemini generateContent ({model}) HTTP 200 without image bytes: {}",
                &txt.chars().take(240).collect::<String>()
            );
        } else {
            tracing::warn!(
                "Gemini generateContent ({model}) HTTP {}: {}",
                status.as_u16(),
                &txt.chars().take(240).collect::<String>()
            );
            // Retry with minimal generationConfig in case imageConfig/imageSize is rejected by model variant
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
                                tracing::info!("Gemini generateContent minimal ({model}) produced infographic image");
                                return Some(img);
                            }
                        }
                    } else {
                        tracing::warn!(
                            "Gemini generateContent minimal ({model}) HTTP {}: {}",
                            st2.as_u16(),
                            &txt2.chars().take(240).collect::<String>()
                        );
                    }
                }
            }
        }
    }

    None
}

async fn call_gemini_interactions(
    client: &Client,
    api_key: &str,
    interactions_url: &str,
    model: &str,
    prompt: &str,
) -> Option<(String, Vec<u8>)> {
    // Interactions API expects lowercase `response_modalities: ["image"]`
    let payload = json!({
        "model": model,
        "input": prompt,
        "response_modalities": ["image"]
    });

    if let Ok(resp) = client
        .post(interactions_url)
        .header("x-goog-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        if status.is_success() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                if let Some(img) = extract_image_from_gemini_json(&parsed) {
                    tracing::info!("Gemini Interactions ({model}) produced infographic image");
                    return Some(img);
                }
            }
            tracing::warn!(
                "Gemini Interactions ({model}) HTTP 200 without image bytes: {}",
                &txt.chars().take(240).collect::<String>()
            );
        } else {
            tracing::warn!(
                "Gemini Interactions ({model}) HTTP {}: {}",
                status.as_u16(),
                &txt.chars().take(240).collect::<String>()
            );
        }
    }

    None
}

async fn request_gemini_raw_image(
    prompt: &str,
    aspect_ratio: &str,
) -> Option<(String, Vec<u8>)> {
    let (api_key, api_url, configured_model) = get_gemini_config()?;
    let client = get_gemini_client();

    let mut models = vec![configured_model.clone()];
    for fallback in [
        "gemini-3.1-flash-lite-image",
        "gemini-3.1-flash-image-preview",
        "gemini-2.5-flash-image",
    ] {
        if !models.iter().any(|m| m == fallback) {
            models.push(fallback.to_string());
        }
    }

    let is_interactions = api_url.to_lowercase().contains("/interactions");

    for model in &models {
        let generate_content_url = if !is_interactions && api_url.contains(":generateContent") {
            api_url.clone()
        } else if !is_interactions && api_url.contains("/models") {
            format!("{}/{}:generateContent", api_url.trim_end_matches('/'), model)
        } else {
            format!("{}/{}:generateContent", STANDARD_GEMINI_MODELS_BASE, model)
        };

        // Note: Google's `gemini-3.1-flash-lite-image` docs mark `/v1beta/interactions`
        // as unsupported and route image models through `:generateContent`.
        if is_interactions && !model.contains("lite") {
            if let Some(img) = call_gemini_interactions(
                client,
                api_key.as_str(),
                api_url.as_str(),
                model.as_str(),
                prompt,
            )
            .await
            {
                return Some(img);
            }
            if let Some(img) = call_gemini_generate_content(
                client,
                api_key.as_str(),
                generate_content_url.as_str(),
                model.as_str(),
                prompt,
                aspect_ratio,
            )
            .await
            {
                return Some(img);
            }
        } else {
            if let Some(img) = call_gemini_generate_content(
                client,
                api_key.as_str(),
                generate_content_url.as_str(),
                model.as_str(),
                prompt,
                aspect_ratio,
            )
            .await
            {
                return Some(img);
            }
            if is_interactions {
                if let Some(img) = call_gemini_interactions(
                    client,
                    api_key.as_str(),
                    api_url.as_str(),
                    model.as_str(),
                    prompt,
                )
                .await
                {
                    return Some(img);
                }
            }
        }
    }

    None
}

/// Generates an educational illustration via Gemini Image API, stores it in PostgreSQL
/// `uploaded_files` + local disk cache, and returns `/uploads/<filename>`.
pub async fn generate_infographic_image_url(
    pool: &PgPool,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
    aspect_ratio: &str,
) -> Option<String> {
    let prompt = format!(
        "Create a vibrant, child-safe educational illustration for an Indonesian school infographic module.\n\
        Subject: {subject_name} | Student Level: {grade_level}\n\
        Visual Scene / Concept: {concept_prompt}\n\
        Art Direction: clean stylized 3D Pixar-inspired textbook illustration, crisp bright lighting, high visual clarity, clear subject focus, NO garbled or random text labels, NO watermarks."
    );

    let (mime, bytes) = request_gemini_raw_image(&prompt, aspect_ratio).await?;

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

    Some(format!("/uploads/{filename}"))
}
