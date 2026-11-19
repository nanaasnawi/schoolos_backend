use reqwest::Client;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_CLOUDFLARE_IMAGE_MODEL: &str = "@cf/black-forest-labs/flux-1-schnell";

static SHARED_CF_CLIENT: OnceLock<Client> = OnceLock::new();

fn get_cloudflare_client() -> &'static Client {
    SHARED_CF_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(45))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

#[derive(Clone, Debug)]
pub struct CloudflareImageConfig {
    pub account_id: String,
    pub api_token: String,
    pub model: String,
}

fn clean_env_secret(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn get_cloudflare_config() -> Result<CloudflareImageConfig, String> {
    let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID")
        .ok()
        .map(|v| clean_env_secret(&v))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Variable CLOUDFLARE_ACCOUNT_ID belum terpasang di Railway".to_string())?;

    let mut api_token = std::env::var("CLOUDFLARE_API_TOKEN")
        .ok()
        .map(|v| clean_env_secret(&v))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Variable CLOUDFLARE_API_TOKEN belum terpasang di Railway".to_string())?;

    // Guard against pasting "Bearer <token>" into Railway variables
    if api_token.len() > 7 && api_token[..7].eq_ignore_ascii_case("bearer ") {
        api_token = api_token[7..].trim().to_string();
    }
    if api_token.is_empty() {
        return Err("Variable CLOUDFLARE_API_TOKEN kosong".to_string());
    }

    let model = std::env::var("CLOUDFLARE_IMAGE_MODEL")
        .ok()
        .map(|m| clean_env_secret(&m))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_CLOUDFLARE_IMAGE_MODEL.to_string());

    Ok(CloudflareImageConfig {
        account_id,
        api_token,
        model,
    })
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

fn build_infographic_visual_prompt(
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
) -> String {
    format!(
        "Pure clean wordless 3D Pixar-style educational illustration for {subject_name} ({grade_level}), visual storytelling only.\n\
        Visual scene: {concept_prompt}\n\
        Style & constraints: vibrant colors, crisp warm lighting, clean composition, blank unlabeled book covers/walls/signs, strictly no text, no typography, no words, no letters, no numbers, no titles, no labels, no captions, no speech bubbles, no watermarks, pure clean illustration."
    )
}

fn extract_cloudflare_error_message(txt: &str, status_code: u16) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(txt) {
        if let Some(msg) = v
            .get("errors")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|err| err.get("message"))
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

async fn call_cloudflare_flux(
    client: &Client,
    cfg: &CloudflareImageConfig,
    prompt: &str,
) -> Result<(String, Vec<u8>), String> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
        cfg.account_id, cfg.model
    );
    let clipped_prompt: String = prompt.chars().take(1900).collect();
    let payload = json!({
        "prompt": clipped_prompt,
        "steps": 6
    });

    let resp = client
        .post(url.as_str())
        .header("Authorization", format!("Bearer {}", cfg.api_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network error ({}): {e}", cfg.model))?;

    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let raw_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed reading response ({}): {e}", cfg.model))?;

    if status.is_success() && content_type.starts_with("image/") && !raw_bytes.is_empty() {
        return Ok((content_type, raw_bytes.to_vec()));
    }

    let txt = String::from_utf8_lossy(&raw_bytes).into_owned();
    if status.is_success() {
        if let Ok(val) = serde_json::from_str::<Value>(&txt) {
            let b64 = val
                .get("result")
                .and_then(|r| r.get("image"))
                .and_then(|i| i.as_str())
                .or_else(|| val.get("image").and_then(|i| i.as_str()))
                .unwrap_or("");
            if let Some(bytes) = decode_base64(b64) {
                return Ok(("image/jpeg".to_string(), bytes));
            }
        }
        return Err(format!("{}: respon 200 tetapi tidak berisi byte gambar", cfg.model));
    }

    let err_msg = extract_cloudflare_error_message(&txt, status.as_u16());
    Err(format!("Cloudflare: {} -> {}", cfg.model, err_msg))
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
        tracing::warn!("Failed to persist AI infographic image to uploaded_files: {e}");
    }

    let _ = tokio::fs::create_dir_all("uploads").await;
    let disk_path = std::path::PathBuf::from("uploads").join(&filename);
    let _ = tokio::fs::write(&disk_path, &bytes).await;

    format!("/uploads/{filename}")
}

/// Generates the Hero cover image via Cloudflare Workers AI (`@cf/black-forest-labs/flux-1-schnell`).
pub async fn generate_hero_image(
    pool: &PgPool,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
) -> Result<(String, CloudflareImageConfig), String> {
    let cfg = get_cloudflare_config()?;
    let client = get_cloudflare_client();
    let prompt = build_infographic_visual_prompt(concept_prompt, subject_name, grade_level);

    let (mime, bytes) = call_cloudflare_flux(client, &cfg, &prompt).await?;
    tracing::info!("Cloudflare Workers AI hero image succeeded ({})", cfg.model);
    let url = persist_image(pool, &mime, bytes).await;
    Ok((url, cfg))
}

/// Fast generation for infographic step cards reusing the verified Cloudflare config.
pub async fn generate_card_image(
    pool: &PgPool,
    cfg: &CloudflareImageConfig,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
) -> Option<String> {
    let client = get_cloudflare_client();
    let prompt = build_infographic_visual_prompt(concept_prompt, subject_name, grade_level);

    match call_cloudflare_flux(client, cfg, &prompt).await {
        Ok((mime, bytes)) => Some(persist_image(pool, &mime, bytes).await),
        Err(err) => {
            tracing::warn!("Cloudflare FLUX card image failed ({}): {}", cfg.model, err);
            None
        }
    }
}

/// Optional wordless illustration generator for non-numeric visual prompts (falls back cleanly to `None` if unconfigured).
pub async fn try_generate_scene_image(
    pool: &PgPool,
    concept_prompt: &str,
    subject_name: &str,
    grade_level: &str,
) -> Option<String> {
    generate_hero_image(pool, concept_prompt, subject_name, grade_level)
        .await
        .ok()
        .map(|(url, _)| url)
}
