use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db_tenant::with_system_rls_bypass;
use crate::infrastructure::cloudflare_ai;
use crate::presentation::learning::materials::dto::learning_material_response::determine_material_format;

/// Validates a candidate image/cover URL and normalizes `uploads/...` relative paths to `/uploads/...`.
pub fn normalize_image_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("unsplash.com") || lower == "null" || lower == "none" {
        return None;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if trimmed.starts_with("/uploads/") {
        return Some(trimmed.to_string());
    }
    if lower.starts_with("uploads/") {
        return Some(format!("/{trimmed}"));
    }
    None
}

fn is_valid_youtube_id(id: &str) -> bool {
    let len = id.len();
    (8..=16).contains(&len)
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Extracts a YouTube video ID from watch, short, embed, shorts, live, or youtu.be URLs
/// (including HTML iframe snippets and URL-encoded query parameters).
pub fn extract_youtube_video_id(raw_url: &str) -> Option<String> {
    let url = raw_url.trim();
    if url.is_empty() {
        return None;
    }
    let decoded = url
        .replace("%3F", "?")
        .replace("%3f", "?")
        .replace("%26", "&")
        .replace("%3D", "=")
        .replace("%3d", "=")
        .replace("%2F", "/")
        .replace("%2f", "/");
    let lower = decoded.to_ascii_lowercase();
    let delims = ['?', '&', '#', '/', '"', '\'', '<', '>', ' ', '\r', '\n', ')', ']', ';', ','];

    // 1. youtu.be/<id>
    if let Some(pos) = lower.find("youtu.be/") {
        let rest = &decoded[pos + "youtu.be/".len()..];
        let id = rest.split(delims).next().unwrap_or("").trim();
        if is_valid_youtube_id(id) {
            return Some(id.to_string());
        }
    }

    // 2. youtube.com/watch?v=<id> or &v=<id>
    if lower.contains("youtube.com") || lower.contains("youtube-nocookie.com") {
        if let Some(query_pos) = decoded.find('?') {
            let query = &decoded[query_pos + 1..];
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k.eq_ignore_ascii_case("v") {
                        let id = v.split(delims).next().unwrap_or("").trim();
                        if is_valid_youtube_id(id) {
                            return Some(id.to_string());
                        }
                    }
                }
            }
        }

        // 3. /embed/<id>, /shorts/<id>, /live/<id>, /v/<id>, /watch/<id>
        for marker in ["/embed/", "/shorts/", "/live/", "/v/", "/watch/"] {
            if let Some(pos) = lower.find(marker) {
                let rest = &decoded[pos + marker.len()..];
                let id = rest.split(delims).next().unwrap_or("").trim();
                if is_valid_youtube_id(id) {
                    return Some(id.to_string());
                }
            }
        }
    }

    None
}

/// Returns YouTube native `hqdefault.jpg` thumbnail URL for any valid YouTube link.
pub fn extract_youtube_thumbnail(raw_url: Option<&str>) -> Option<String> {
    let url = raw_url?;
    let video_id = extract_youtube_video_id(url)?;
    Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"))
}

fn find_first_markdown_image_url(text: &str) -> Option<String> {
    let mut search = text;
    while let Some(open) = search.find("![") {
        let after_bang = &search[open + 2..];
        if let Some(close_bracket) = after_bang.find("](") {
            let after_paren = &after_bang[close_bracket + 2..];
            if let Some(close_paren) = after_paren.find(')') {
                let candidate = &after_paren[..close_paren];
                let clean_url = candidate.split_whitespace().next().unwrap_or(candidate);
                if let Some(norm) = normalize_image_url(clean_url) {
                    return Some(norm);
                }
            }
        }
        search = &search[open + 2..];
    }
    None
}

fn looks_like_raster_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let without_query = lower.split(['?', '#']).next().unwrap_or(&lower);
    if without_query.ends_with(".pdf") || without_query.ends_with(".svg") {
        return false;
    }
    without_query.starts_with("/uploads/")
        || without_query.ends_with(".png")
        || without_query.ends_with(".jpg")
        || without_query.ends_with(".jpeg")
        || without_query.ends_with(".webp")
        || without_query.ends_with(".gif")
}

/// Extracts the magazine cover / primary raster hero image from an infographic material
/// without invoking FLUX AI again. Returns `None` if no raster cover exists so FLUX AI
/// can generate one once on publish.
pub fn extract_infographic_cover_thumbnail(
    description: Option<&str>,
    external_url: Option<&str>,
) -> Option<String> {
    if let Some(ext) = external_url.and_then(normalize_image_url) {
        if looks_like_raster_image_url(&ext) {
            return Some(ext);
        }
    }

    let raw_desc = description?.trim();
    if raw_desc.is_empty() {
        return None;
    }

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_desc) {
        // Object wrapper: check top-level cover/hero fields first, then blocks array.
        if let Some(obj) = val.as_object() {
            for key in ["hero_url", "cover_url", "cover_image", "thumbnail_url", "image_url"] {
                if let Some(s) = obj
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(normalize_image_url)
                    .filter(|u| looks_like_raster_image_url(u))
                {
                    return Some(s);
                }
            }
            if let Some(arr) = obj.get("blocks").and_then(|b| b.as_array()) {
                if let Some(found) = extract_cover_from_blocks(arr) {
                    return Some(found);
                }
            }
        }
        // Array of InfographicBlockDto items stored directly in description.
        if let Some(arr) = val.as_array() {
            if let Some(found) = extract_cover_from_blocks(arr) {
                return Some(found);
            }
        }
    }

    if let Some(md_url) = find_first_markdown_image_url(raw_desc) {
        if looks_like_raster_image_url(&md_url) {
            return Some(md_url);
        }
    }

    None
}

fn extract_cover_from_blocks(blocks: &[serde_json::Value]) -> Option<String> {
    let mut first_image: Option<String> = None;

    for block in blocks {
        let b_type = block
            .get("type")
            .or_else(|| block.get("block_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if !b_type.eq_ignore_ascii_case("IMAGE") {
            continue;
        }
        let Some(url) = block
            .get("content")
            .and_then(|v| v.as_str())
            .and_then(normalize_image_url)
            .filter(|u| looks_like_raster_image_url(u))
        else {
            continue;
        };

        let id_str = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id_str.contains("hero-cover") || url.contains("ai_infografis") {
            return Some(url);
        }
        if first_image.is_none() {
            first_image = Some(url);
        }
    }

    first_image
}

/// Verifies `/uploads/...` URLs exist on disk or in `uploaded_files` so stale local links
/// automatically fall back to 1x FLUX generation instead of returning 404 images.
async fn verify_deterministic_thumbnail_usable(pool: &PgPool, url: String) -> Option<String> {
    if !url.starts_with("/uploads/") {
        return Some(url);
    }
    let clean_name = std::path::Path::new(&url)
        .file_name()
        .and_then(|f| f.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let disk_path = std::path::PathBuf::from("uploads").join(clean_name);
    if disk_path.is_file() {
        return Some(url);
    }
    let in_db: bool = with_system_rls_bypass(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM uploaded_files WHERE filename = $1)",
        )
        .bind(clean_name)
        .fetch_one(pool),
    )
    .await
    .unwrap_or(false);
    if in_db {
        Some(url)
    } else {
        None
    }
}

/// Resolves deterministic zero-AI thumbnails for VIDEO and INFOGRAPHIC materials.
pub fn resolve_deterministic_material_thumbnail(
    material_type: &str,
    external_url: Option<&str>,
    description: Option<&str>,
    source_type: Option<&str>,
    library_book_id: Option<Uuid>,
) -> Option<String> {
    let fmt = determine_material_format(
        material_type,
        external_url,
        description,
        source_type,
        library_book_id,
    );
    match fmt.as_str() {
        "VIDEO" => {
            extract_youtube_thumbnail(external_url).or_else(|| extract_youtube_thumbnail(description))
        }
        "INFOGRAPHIC" => extract_infographic_cover_thumbnail(description, external_url),
        _ => None,
    }
}

/// Strips JSON/markdown noise before building an AI thumbnail scene prompt.
pub fn summarize_context_for_prompt(raw: Option<&str>) -> Option<String> {
    let txt = raw?.trim();
    if txt.is_empty() || txt.starts_with('[') || txt.starts_with('{') {
        return None;
    }
    let clean: String = txt
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("!["))
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(280)
        .collect();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

/// Resolves deterministic thumbnails immediately or spawns a 1x FLUX generation task when a
/// material is published. Never runs on draft/scheduled items or student GET requests.
pub async fn ensure_material_thumbnail_on_publish(pool: PgPool, material_id: Uuid) -> Option<String> {
    let row = with_system_rls_bypass(
        sqlx::query(
            r#"
            SELECT
                m.id, m.tenant_id, m.material_type, m.title, m.description,
                COALESCE(
                    NULLIF(TRIM(m.external_url), ''),
                    CASE
                        WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                        WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                        ELSE NULL
                    END,
                    lb.file_url
                ) AS resolved_external_url,
                m.source_type, m.library_book_id,
                m.thumbnail_url, m.thumbnail_status,
                m.visibility, m.is_active, m.release_at,
                COALESCE(s.name, lb.subject_name, t.subject, 'Umum') AS subject_name,
                COALESCE(c.name, 'SMA/Paket C') AS class_name,
                lb.cover_url AS library_cover_url
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            WHERE m.id = $1 AND m.deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(material_id)
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()?;

    let existing_thumb = row
        .try_get::<Option<String>, _>("thumbnail_url")
        .ok()
        .flatten()
        .and_then(|s| normalize_image_url(&s));

    let tenant_id: Uuid = row.get("tenant_id");
    let material_type: String = row.get("material_type");
    let title: String = row.get("title");
    let description: Option<String> = row.try_get("description").ok().flatten();
    let external_url: Option<String> = row.try_get("resolved_external_url").ok().flatten();
    let source_type: Option<String> = row.try_get("source_type").ok().flatten();
    let library_book_id: Option<Uuid> = row.try_get("library_book_id").ok().flatten();
    let thumbnail_status: String = row
        .try_get::<String, _>("thumbnail_status")
        .unwrap_or_else(|_| "pending".to_string());
    let visibility: String = row
        .try_get::<String, _>("visibility")
        .unwrap_or_else(|_| "published".to_string());
    let is_active: bool = row.try_get("is_active").unwrap_or(true);
    let release_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("release_at").ok().flatten();
    let subject_name: String = row
        .try_get::<String, _>("subject_name")
        .unwrap_or_else(|_| "Umum".to_string());
    let class_name: String = row
        .try_get::<String, _>("class_name")
        .unwrap_or_else(|_| "Umum".to_string());
    let library_cover_url: Option<String> = row
        .try_get::<Option<String>, _>("library_cover_url")
        .ok()
        .flatten()
        .and_then(|u| normalize_image_url(&u));

    let fmt = determine_material_format(
        &material_type,
        external_url.as_deref(),
        description.as_deref(),
        source_type.as_deref(),
        library_book_id,
    );

    // 1. Instant zero-token resolution for Video and Infografis with verified existing assets.
    let deterministic_candidate = resolve_deterministic_material_thumbnail(
        &material_type,
        external_url.as_deref(),
        description.as_deref(),
        source_type.as_deref(),
        library_book_id,
    );
    if let Some(det) = deterministic_candidate {
        if let Some(deterministic_url) = verify_deterministic_thumbnail_usable(&pool, det).await {
            if existing_thumb.as_deref() != Some(deterministic_url.as_str()) {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        r#"
                        UPDATE learning_materials
                        SET thumbnail_url = $1,
                            thumbnail_status = 'ready'
                        WHERE id = $2
                        "#,
                    )
                    .bind(&deterministic_url)
                    .bind(material_id)
                    .execute(&pool),
                )
                .await;
            }
            return Some(deterministic_url);
        }
    }

    if let Some(existing) = existing_thumb {
        return Some(existing);
    }

    // Never invoke FLUX on drafts, archived items, or future-scheduled materials.
    let is_live = is_active
        && !visibility.eq_ignore_ascii_case("archived")
        && release_at.is_none_or(|r| r <= chrono::Utc::now())
        && (visibility.eq_ignore_ascii_case("published")
            || visibility.eq_ignore_ascii_case("active")
            || release_at.is_some_and(|r| r <= chrono::Utc::now()));
    if !is_live || thumbnail_status != "pending" {
        return None;
    }

    // 2. Reuse existing cached thumbnail from a multi-rombel sibling or same library book (0 AI calls).
    let sibling_url: Option<String> = with_system_rls_bypass(
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT thumbnail_url
            FROM learning_materials
            WHERE tenant_id = $1
              AND id != $2
              AND deleted_at IS NULL
              AND thumbnail_status = 'ready'
              AND thumbnail_url IS NOT NULL
              AND BTRIM(thumbnail_url) != ''
              AND (
                  ($3::uuid IS NOT NULL AND library_book_id = $3)
                  OR (LOWER(TRIM(title)) = LOWER(TRIM($4)) AND LOWER(TRIM(material_type)) = LOWER(TRIM($5)))
              )
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(material_id)
        .bind(library_book_id)
        .bind(&title)
        .bind(&material_type)
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()
    .flatten();

    if let Some(reused) = sibling_url.and_then(|u| normalize_image_url(&u)) {
        propagate_material_thumbnail_to_siblings(
            &pool,
            tenant_id,
            material_id,
            library_book_id,
            &title,
            &material_type,
            &reused,
        )
        .await;
        return Some(reused);
    }

    if !cloudflare_ai::is_configured() {
        if let Some(fallback_cover) = library_cover_url {
            propagate_material_thumbnail_to_siblings(
                &pool,
                tenant_id,
                material_id,
                library_book_id,
                &title,
                &material_type,
                &fallback_cover,
            )
            .await;
            return Some(fallback_cover);
        }
        return None;
    }

    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let _ = run_single_material_flux_generation(
            &pool_clone,
            material_id,
            tenant_id,
            library_book_id,
            material_type,
            fmt,
            title,
            subject_name,
            class_name,
            description,
            library_cover_url,
        )
        .await;
    });

    None
}

async fn propagate_material_thumbnail_to_siblings(
    pool: &PgPool,
    tenant_id: Uuid,
    material_id: Uuid,
    library_book_id: Option<Uuid>,
    title: &str,
    material_type: &str,
    url: &str,
) {
    let _ = with_system_rls_bypass(
        sqlx::query(
            r#"
            UPDATE learning_materials
            SET thumbnail_url = $1,
                thumbnail_status = 'ready'
            WHERE (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (
                  id = $2
                  OR (
                      tenant_id = $3
                      AND deleted_at IS NULL
                      AND (
                          ($4::uuid IS NOT NULL AND library_book_id = $4)
                          OR (LOWER(TRIM(title)) = LOWER(TRIM($5)) AND LOWER(TRIM(material_type)) = LOWER(TRIM($6)))
                      )
                  )
              )
            "#,
        )
        .bind(url)
        .bind(material_id)
        .bind(tenant_id)
        .bind(library_book_id)
        .bind(title)
        .bind(material_type)
        .execute(pool),
    )
    .await;
}

async fn propagate_assignment_thumbnail_to_siblings(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
    title: &str,
    url: &str,
) {
    let _ = with_system_rls_bypass(
        sqlx::query(
            r#"
            UPDATE assignments
            SET thumbnail_url = $1,
                thumbnail_status = 'ready'
            WHERE (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (
                  id = $2
                  OR (
                      tenant_id = $3
                      AND deleted_at IS NULL
                      AND LOWER(TRIM(title)) = LOWER(TRIM($4))
                  )
              )
            "#,
        )
        .bind(url)
        .bind(assignment_id)
        .bind(tenant_id)
        .bind(title)
        .execute(pool),
    )
    .await;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FluxAttemptOutcome {
    Ready,
    Transient,
    FailedOrSkipped,
}

async fn run_single_material_flux_generation(
    pool: &PgPool,
    material_id: Uuid,
    tenant_id: Uuid,
    library_book_id: Option<Uuid>,
    material_type: String,
    fmt: String,
    title: String,
    subject_name: String,
    class_name: String,
    description: Option<String>,
    library_cover_url: Option<String>,
) -> FluxAttemptOutcome {
    let sibling_ready: Option<String> = with_system_rls_bypass(
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT thumbnail_url
            FROM learning_materials
            WHERE tenant_id = $1
              AND id != $2
              AND deleted_at IS NULL
              AND thumbnail_status = 'ready'
              AND thumbnail_url IS NOT NULL
              AND BTRIM(thumbnail_url) != ''
              AND (
                  ($3::uuid IS NOT NULL AND library_book_id = $3)
                  OR (LOWER(TRIM(title)) = LOWER(TRIM($4)) AND LOWER(TRIM(material_type)) = LOWER(TRIM($5)))
              )
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(material_id)
        .bind(library_book_id)
        .bind(&title)
        .bind(&material_type)
        .fetch_optional(pool),
    )
    .await
    .ok()
    .flatten()
    .flatten();

    if let Some(reused) = sibling_ready.and_then(|u| normalize_image_url(&u)) {
        propagate_material_thumbnail_to_siblings(
            pool,
            tenant_id,
            material_id,
            library_book_id,
            &title,
            &material_type,
            &reused,
        )
        .await;
        return FluxAttemptOutcome::Ready;
    }

    // Prevent concurrent sibling rombels (e.g. 12A and 12B) from issuing duplicate FLUX requests.
    let sibling_processing: bool = with_system_rls_bypass(
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE tenant_id = $1
                  AND id != $2
                  AND deleted_at IS NULL
                  AND thumbnail_status = 'processing'
                  AND (
                      ($3::uuid IS NOT NULL AND library_book_id = $3)
                      OR (LOWER(TRIM(title)) = LOWER(TRIM($4)) AND LOWER(TRIM(material_type)) = LOWER(TRIM($5)))
                  )
            )
            "#,
        )
        .bind(tenant_id)
        .bind(material_id)
        .bind(library_book_id)
        .bind(&title)
        .bind(&material_type)
        .fetch_one(pool),
    )
    .await
    .unwrap_or(false);

    if sibling_processing {
        return FluxAttemptOutcome::FailedOrSkipped;
    }

    // Atomic compare-and-set claim guarantees FLUX runs AT MOST ONCE per material group.
    let claimed = with_system_rls_bypass(
        sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE learning_materials
            SET thumbnail_status = 'processing'
            WHERE id = $1
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
            RETURNING id
            "#,
        )
        .bind(material_id)
        .fetch_optional(pool),
    )
    .await
    .ok()
    .flatten()
    .is_some();

    if !claimed {
        return FluxAttemptOutcome::FailedOrSkipped;
    }

    let snippet = summarize_context_for_prompt(description.as_deref());
    let res = with_system_rls_bypass(cloudflare_ai::generate_module_thumbnail_image(
        pool,
        &fmt,
        &title,
        &subject_name,
        &class_name,
        snippet.as_deref(),
    ))
    .await;

    match res {
        Ok(url) => {
            propagate_material_thumbnail_to_siblings(
                pool,
                tenant_id,
                material_id,
                library_book_id,
                &title,
                &material_type,
                &url,
            )
            .await;
            FluxAttemptOutcome::Ready
        }
        Err(cloudflare_ai::ThumbnailGenError::NotConfigured)
        | Err(cloudflare_ai::ThumbnailGenError::Transient(_)) => {
            let _ = with_system_rls_bypass(
                sqlx::query(
                    "UPDATE learning_materials SET thumbnail_status = 'pending' WHERE id = $1 AND thumbnail_status = 'processing'",
                )
                .bind(material_id)
                .execute(pool),
            )
            .await;
            FluxAttemptOutcome::Transient
        }
        Err(cloudflare_ai::ThumbnailGenError::Failed(err)) => {
            tracing::warn!(
                material_id = %material_id,
                "FLUX thumbnail generation failed cleanly: {err}"
            );
            if let Some(fallback_cover) = library_cover_url {
                propagate_material_thumbnail_to_siblings(
                    pool,
                    tenant_id,
                    material_id,
                    library_book_id,
                    &title,
                    &material_type,
                    &fallback_cover,
                )
                .await;
                FluxAttemptOutcome::Ready
            } else {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        "UPDATE learning_materials SET thumbnail_status = 'failed' WHERE id = $1 AND thumbnail_status = 'processing'",
                    )
                    .bind(material_id)
                    .execute(pool),
                )
                .await;
                FluxAttemptOutcome::FailedOrSkipped
            }
        }
    }
}

async fn run_single_assignment_flux_generation(
    pool: &PgPool,
    assignment_id: Uuid,
    tenant_id: Uuid,
    title: String,
    subject_name: String,
    class_name: String,
    context: Option<String>,
) -> FluxAttemptOutcome {
    let sibling_ready: Option<String> = with_system_rls_bypass(
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT thumbnail_url
            FROM assignments
            WHERE tenant_id = $1
              AND id != $2
              AND deleted_at IS NULL
              AND thumbnail_status = 'ready'
              AND thumbnail_url IS NOT NULL
              AND BTRIM(thumbnail_url) != ''
              AND LOWER(TRIM(title)) = LOWER(TRIM($3))
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(&title)
        .fetch_optional(pool),
    )
    .await
    .ok()
    .flatten()
    .flatten();

    if let Some(reused) = sibling_ready.and_then(|u| normalize_image_url(&u)) {
        propagate_assignment_thumbnail_to_siblings(pool, tenant_id, assignment_id, &title, &reused)
            .await;
        return FluxAttemptOutcome::Ready;
    }

    let sibling_processing: bool = with_system_rls_bypass(
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM assignments
                WHERE tenant_id = $1
                  AND id != $2
                  AND deleted_at IS NULL
                  AND thumbnail_status = 'processing'
                  AND LOWER(TRIM(title)) = LOWER(TRIM($3))
            )
            "#,
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(&title)
        .fetch_one(pool),
    )
    .await
    .unwrap_or(false);

    if sibling_processing {
        return FluxAttemptOutcome::FailedOrSkipped;
    }

    let claimed = with_system_rls_bypass(
        sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE assignments
            SET thumbnail_status = 'processing'
            WHERE id = $1
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
            RETURNING id
            "#,
        )
        .bind(assignment_id)
        .fetch_optional(pool),
    )
    .await
    .ok()
    .flatten()
    .is_some();

    if !claimed {
        return FluxAttemptOutcome::FailedOrSkipped;
    }

    let res = with_system_rls_bypass(cloudflare_ai::generate_module_thumbnail_image(
        pool,
        "ASSIGNMENT",
        &title,
        &subject_name,
        &class_name,
        context.as_deref(),
    ))
    .await;

    match res {
        Ok(url) => {
            propagate_assignment_thumbnail_to_siblings(
                pool,
                tenant_id,
                assignment_id,
                &title,
                &url,
            )
            .await;
            FluxAttemptOutcome::Ready
        }
        Err(cloudflare_ai::ThumbnailGenError::NotConfigured)
        | Err(cloudflare_ai::ThumbnailGenError::Transient(_)) => {
            let _ = with_system_rls_bypass(
                sqlx::query(
                    "UPDATE assignments SET thumbnail_status = 'pending' WHERE id = $1 AND thumbnail_status = 'processing'",
                )
                .bind(assignment_id)
                .execute(pool),
            )
            .await;
            FluxAttemptOutcome::Transient
        }
        Err(cloudflare_ai::ThumbnailGenError::Failed(err)) => {
            tracing::warn!(
                assignment_id = %assignment_id,
                "FLUX assignment thumbnail generation failed cleanly: {err}"
            );
            let _ = with_system_rls_bypass(
                sqlx::query(
                    "UPDATE assignments SET thumbnail_status = 'failed' WHERE id = $1 AND thumbnail_status = 'processing'",
                )
                .bind(assignment_id)
                .execute(pool),
            )
            .await;
            FluxAttemptOutcome::FailedOrSkipped
        }
    }
}

/// Spawns a 1x FLUX thumbnail task when an assignment is published (never on drafts or GET reads).
pub async fn ensure_assignment_thumbnail_on_publish(pool: PgPool, assignment_id: Uuid) -> Option<String> {
    let row = with_system_rls_bypass(
        sqlx::query(
            r#"
            SELECT
                a.id, a.tenant_id, a.title, a.description, a.instructions,
                a.status, a.is_active, a.release_at,
                a.thumbnail_url, a.thumbnail_status,
                COALESCE(sub.name, t.subject, 'Umum') AS subject_name,
                COALESCE(c.name, 'SMA/Paket C') AS class_name
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.id = $1 AND a.deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(assignment_id)
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()?;

    if let Some(existing) = row
        .try_get::<Option<String>, _>("thumbnail_url")
        .ok()
        .flatten()
        .and_then(|s| normalize_image_url(&s))
    {
        return Some(existing);
    }

    let tenant_id: Uuid = row.get("tenant_id");
    let title: String = row.get("title");
    let description: Option<String> = row.try_get("description").ok().flatten();
    let instructions: Option<String> = row.try_get("instructions").ok().flatten();
    let status: String = row.try_get("status").unwrap_or_else(|_| "draft".to_string());
    let is_active: bool = row.try_get("is_active").unwrap_or(true);
    let release_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("release_at").ok().flatten();
    let thumbnail_status: String = row
        .try_get::<String, _>("thumbnail_status")
        .unwrap_or_else(|_| "pending".to_string());
    let subject_name: String = row
        .try_get::<String, _>("subject_name")
        .unwrap_or_else(|_| "Umum".to_string());
    let class_name: String = row
        .try_get::<String, _>("class_name")
        .unwrap_or_else(|_| "Umum".to_string());

    let is_live = is_active
        && !status.eq_ignore_ascii_case("archived")
        && release_at.is_none_or(|r| r <= chrono::Utc::now())
        && (status.eq_ignore_ascii_case("published")
            || status.eq_ignore_ascii_case("active")
            || status.eq_ignore_ascii_case("closed")
            || release_at.is_some_and(|r| r <= chrono::Utc::now()));
    if !is_live || thumbnail_status != "pending" {
        return None;
    }

    let sibling_url: Option<String> = with_system_rls_bypass(
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT thumbnail_url
            FROM assignments
            WHERE tenant_id = $1
              AND id != $2
              AND deleted_at IS NULL
              AND thumbnail_status = 'ready'
              AND thumbnail_url IS NOT NULL
              AND BTRIM(thumbnail_url) != ''
              AND LOWER(TRIM(title)) = LOWER(TRIM($3))
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(&title)
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()
    .flatten();

    if let Some(reused) = sibling_url.and_then(|u| normalize_image_url(&u)) {
        propagate_assignment_thumbnail_to_siblings(&pool, tenant_id, assignment_id, &title, &reused)
            .await;
        return Some(reused);
    }

    if !cloudflare_ai::is_configured() {
        return None;
    }

    let context = summarize_context_for_prompt(description.as_deref().or(instructions.as_deref()));
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let _ = run_single_assignment_flux_generation(
            &pool_clone,
            assignment_id,
            tenant_id,
            title,
            subject_name,
            class_name,
            context,
        )
        .await;
    });

    None
}

async fn propagate_all_ready_siblings_sql(pool: &PgPool) {
    let _ = with_system_rls_bypass(
        sqlx::query(
            r#"
            UPDATE learning_materials m
            SET thumbnail_url = src.thumbnail_url,
                thumbnail_status = 'ready'
            FROM (
                SELECT DISTINCT ON (tenant_id, LOWER(TRIM(title)), LOWER(TRIM(material_type)))
                    tenant_id,
                    LOWER(TRIM(title)) AS norm_title,
                    LOWER(TRIM(material_type)) AS norm_type,
                    library_book_id,
                    thumbnail_url
                FROM learning_materials
                WHERE deleted_at IS NULL
                  AND thumbnail_status = 'ready'
                  AND thumbnail_url IS NOT NULL
                  AND BTRIM(thumbnail_url) != ''
                ORDER BY tenant_id, LOWER(TRIM(title)), LOWER(TRIM(material_type)), updated_at DESC
            ) src
            WHERE m.tenant_id = src.tenant_id
              AND m.deleted_at IS NULL
              AND (m.thumbnail_url IS NULL OR BTRIM(m.thumbnail_url) = '')
              AND (
                  (m.library_book_id IS NOT NULL AND m.library_book_id = src.library_book_id)
                  OR (
                      LOWER(TRIM(m.title)) = src.norm_title
                      AND LOWER(TRIM(m.material_type)) = src.norm_type
                  )
              )
            "#,
        )
        .execute(pool),
    )
    .await;

    let _ = with_system_rls_bypass(
        sqlx::query(
            r#"
            UPDATE assignments a
            SET thumbnail_url = src.thumbnail_url,
                thumbnail_status = 'ready'
            FROM (
                SELECT DISTINCT ON (tenant_id, LOWER(TRIM(title)))
                    tenant_id,
                    LOWER(TRIM(title)) AS norm_title,
                    thumbnail_url
                FROM assignments
                WHERE deleted_at IS NULL
                  AND thumbnail_status = 'ready'
                  AND thumbnail_url IS NOT NULL
                  AND BTRIM(thumbnail_url) != ''
                ORDER BY tenant_id, LOWER(TRIM(title)), updated_at DESC
            ) src
            WHERE a.tenant_id = src.tenant_id
              AND a.deleted_at IS NULL
              AND (a.thumbnail_url IS NULL OR BTRIM(a.thumbnail_url) = '')
              AND LOWER(TRIM(a.title)) = src.norm_title
            "#,
        )
        .execute(pool),
    )
    .await;
}

async fn backfill_published_thumbnails_with_limits_inner(
    pool: &PgPool,
    max_materials: i64,
    max_assignments: i64,
) -> (usize, bool) {
    // 0. Zero-token propagation from already-ready siblings (including multi-rombel copies).
    propagate_all_ready_siblings_sql(pool).await;

    // 1. Zero-token deterministic backfill for VIDEO & verified INFOGRAPHIC materials.
    if let Ok(mat_rows) = with_system_rls_bypass(
        sqlx::query(
            r#"
            SELECT id, tenant_id, title, material_type, description, source_type, library_book_id,
                   COALESCE(
                       NULLIF(TRIM(external_url), ''),
                       CASE
                           WHEN storage_key LIKE '/uploads/%' OR storage_key LIKE 'http%' THEN storage_key
                           WHEN storage_key LIKE 'uploads/%' THEN '/' || storage_key
                           ELSE NULL
                       END
                   ) AS resolved_external_url
            FROM learning_materials
            WHERE deleted_at IS NULL
              AND is_active = true
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND thumbnail_status = 'pending'
              AND (
                  LOWER(COALESCE(material_type, '')) IN ('video', 'image', 'infographic', 'infografis')
                  OR LOWER(COALESCE(external_url, '')) LIKE '%youtu%'
                  OR LOWER(COALESCE(description, '')) LIKE '%youtu%'
                  OR COALESCE(description, '') LIKE '%![%'
                  OR LTRIM(COALESCE(description, '')) LIKE '[%'
              )
            ORDER BY updated_at DESC
            LIMIT 250
            "#,
        )
        .fetch_all(pool),
    )
    .await
    {
        for r in mat_rows {
            let id: Uuid = r.get("id");
            let tenant_id: Uuid = r.get("tenant_id");
            let title: String = r.get("title");
            let m_type: String = r.get("material_type");
            let desc: Option<String> = r.try_get("description").ok().flatten();
            let src_type: Option<String> = r.try_get("source_type").ok().flatten();
            let lib_id: Option<Uuid> = r.try_get("library_book_id").ok().flatten();
            let ext_url: Option<String> = r.try_get("resolved_external_url").ok().flatten();

            if let Some(det) = resolve_deterministic_material_thumbnail(
                &m_type,
                ext_url.as_deref(),
                desc.as_deref(),
                src_type.as_deref(),
                lib_id,
            ) {
                if let Some(url) = verify_deterministic_thumbnail_usable(pool, det).await {
                    propagate_material_thumbnail_to_siblings(
                        pool, tenant_id, id, lib_id, &title, &m_type, &url,
                    )
                    .await;
                }
            }
        }
    }

    if !cloudflare_ai::is_configured() {
        return (0, false);
    }

    let mut processed = 0usize;
    let mut hit_rate_limit = false;

    // 2. Sequential awaited FLUX generation grouped by unique (tenant, title, type) so parallel
    // rombels (e.g. 12A and 12B) NEVER spend duplicate FLUX tokens.
    if let Ok(mat_rows) = with_system_rls_bypass(
        sqlx::query(
            r#"
            SELECT
                m.id, m.tenant_id, m.material_type, m.title, m.description,
                COALESCE(
                    NULLIF(TRIM(m.external_url), ''),
                    CASE
                        WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                        WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                        ELSE NULL
                    END,
                    lb.file_url
                ) AS resolved_external_url,
                m.source_type, m.library_book_id,
                COALESCE(s.name, lb.subject_name, t.subject, 'Umum') AS subject_name,
                COALESCE(c.name, 'SMA/Paket C') AS class_name,
                lb.cover_url AS library_cover_url
            FROM (
                SELECT DISTINCT ON (tenant_id, LOWER(TRIM(title)), LOWER(TRIM(material_type)))
                    id, updated_at
                FROM learning_materials
                WHERE deleted_at IS NULL
                  AND is_active = true
                  AND thumbnail_status = 'pending'
                  AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                  AND (release_at IS NULL OR release_at <= NOW())
                  AND LOWER(COALESCE(visibility, 'published')) NOT IN ('draft', 'scheduled', 'archived')
                ORDER BY tenant_id, LOWER(TRIM(title)), LOWER(TRIM(material_type)), updated_at DESC
            ) picked
            JOIN learning_materials m ON m.id = picked.id
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            ORDER BY picked.updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(max_materials)
        .fetch_all(pool),
    )
    .await
    {
        for r in mat_rows {
            let id: Uuid = r.get("id");
            let tenant_id: Uuid = r.get("tenant_id");
            let m_type: String = r.get("material_type");
            let title: String = r.get("title");
            let desc: Option<String> = r.try_get("description").ok().flatten();
            let ext_url: Option<String> = r.try_get("resolved_external_url").ok().flatten();
            let src_type: Option<String> = r.try_get("source_type").ok().flatten();
            let lib_id: Option<Uuid> = r.try_get("library_book_id").ok().flatten();
            let subject_name: String = r
                .try_get::<String, _>("subject_name")
                .unwrap_or_else(|_| "Umum".to_string());
            let class_name: String = r
                .try_get::<String, _>("class_name")
                .unwrap_or_else(|_| "SMA/Paket C".to_string());
            let lib_cover: Option<String> = r
                .try_get::<Option<String>, _>("library_cover_url")
                .ok()
                .flatten()
                .and_then(|u| normalize_image_url(&u));

            if let Some(det) = resolve_deterministic_material_thumbnail(
                &m_type,
                ext_url.as_deref(),
                desc.as_deref(),
                src_type.as_deref(),
                lib_id,
            ) {
                if let Some(det_url) = verify_deterministic_thumbnail_usable(pool, det).await {
                    propagate_material_thumbnail_to_siblings(
                        pool, tenant_id, id, lib_id, &title, &m_type, &det_url,
                    )
                    .await;
                    processed += 1;
                    continue;
                }
            }

            let fmt = determine_material_format(
                &m_type,
                ext_url.as_deref(),
                desc.as_deref(),
                src_type.as_deref(),
                lib_id,
            );

            match run_single_material_flux_generation(
                pool,
                id,
                tenant_id,
                lib_id,
                m_type,
                fmt,
                title,
                subject_name,
                class_name,
                desc,
                lib_cover,
            )
            .await
            {
                FluxAttemptOutcome::Ready => processed += 1,
                FluxAttemptOutcome::Transient => {
                    hit_rate_limit = true;
                    break;
                }
                FluxAttemptOutcome::FailedOrSkipped => {}
            }
            tokio::time::sleep(std::time::Duration::from_millis(650)).await;
        }
    }

    // 3. Sequential awaited FLUX generation grouped by unique assignment title (published / active / closed).
    if !hit_rate_limit {
        if let Ok(asg_rows) = with_system_rls_bypass(
            sqlx::query(
                r#"
                SELECT
                    a.id, a.tenant_id, a.title, a.description, a.instructions,
                    COALESCE(sub.name, t.subject, 'Umum') AS subject_name,
                    COALESCE(c.name, 'SMA/Paket C') AS class_name
                FROM (
                    SELECT DISTINCT ON (tenant_id, LOWER(TRIM(title)))
                        id, updated_at
                    FROM assignments
                    WHERE deleted_at IS NULL
                      AND is_active = true
                      AND thumbnail_status = 'pending'
                      AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                      AND (release_at IS NULL OR release_at <= NOW())
                      AND LOWER(COALESCE(status, 'published')) NOT IN ('draft', 'scheduled', 'archived')
                    ORDER BY tenant_id, LOWER(TRIM(title)), updated_at DESC
                ) picked
                JOIN assignments a ON a.id = picked.id
                LEFT JOIN classes c ON c.id = a.class_id
                LEFT JOIN subjects sub ON sub.id = a.subject_id
                LEFT JOIN teachers t ON t.id = a.teacher_id
                ORDER BY picked.updated_at DESC
                LIMIT $1
                "#,
            )
            .bind(max_assignments)
            .fetch_all(pool),
        )
        .await
        {
            for r in asg_rows {
                let id: Uuid = r.get("id");
                let tenant_id: Uuid = r.get("tenant_id");
                let title: String = r.get("title");
                let description: Option<String> = r.try_get("description").ok().flatten();
                let instructions: Option<String> = r.try_get("instructions").ok().flatten();
                let subject_name: String = r
                    .try_get::<String, _>("subject_name")
                    .unwrap_or_else(|_| "Umum".to_string());
                let class_name: String = r
                    .try_get::<String, _>("class_name")
                    .unwrap_or_else(|_| "SMA/Paket C".to_string());
                let context =
                    summarize_context_for_prompt(description.as_deref().or(instructions.as_deref()));

                match run_single_assignment_flux_generation(
                    pool,
                    id,
                    tenant_id,
                    title,
                    subject_name,
                    class_name,
                    context,
                )
                .await
                {
                    FluxAttemptOutcome::Ready => processed += 1,
                    FluxAttemptOutcome::Transient => {
                        hit_rate_limit = true;
                        break;
                    }
                    FluxAttemptOutcome::FailedOrSkipped => {}
                }
                tokio::time::sleep(std::time::Duration::from_millis(650)).await;
            }
        }
    }

    (processed, hit_rate_limit)
}

/// Runs a bounded synchronous/sequential backfill pass over published materials & assignments.
/// Deduplicates across sibling rombels before calling FLUX, and immediately propagates each
/// generated cover to every matching rombel copy. Returns count of unique FLUX groups attempted.
pub async fn backfill_published_thumbnails_with_limits(
    pool: &PgPool,
    max_materials: i64,
    max_assignments: i64,
) -> usize {
    backfill_published_thumbnails_with_limits_inner(pool, max_materials, max_assignments)
        .await
        .0
}

async fn count_remaining_pending_thumbnails(pool: &PgPool) -> i64 {
    let mat_pending: i64 = with_system_rls_bypass(
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM learning_materials
            WHERE deleted_at IS NULL
              AND is_active = true
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (release_at IS NULL OR release_at <= NOW())
              AND LOWER(COALESCE(visibility, 'published')) NOT IN ('draft', 'scheduled', 'archived')
            "#,
        )
        .fetch_one(pool),
    )
    .await
    .unwrap_or(0);

    let asg_pending: i64 = with_system_rls_bypass(
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM assignments
            WHERE deleted_at IS NULL
              AND is_active = true
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (release_at IS NULL OR release_at <= NOW())
              AND LOWER(COALESCE(status, 'published')) NOT IN ('draft', 'scheduled', 'archived')
            "#,
        )
        .fetch_one(pool),
    )
    .await
    .unwrap_or(0);

    mat_pending + asg_pending
}

/// Runs once per worker cycle for newly published / pending items without quota spikes.
/// Also gently re-queues up to 2 stale `'failed'` items per table when pending queue is empty.
pub async fn backfill_published_thumbnails_batch(pool: &PgPool) {
    let (progressed, _) = backfill_published_thumbnails_with_limits_inner(pool, 6, 6).await;
    if progressed == 0 && cloudflare_ai::is_configured() {
        let remaining = count_remaining_pending_thumbnails(pool).await;
        if remaining == 0 {
            let _ = with_system_rls_bypass(
                sqlx::query(
                    r#"
                    UPDATE learning_materials
                    SET thumbnail_status = 'pending'
                    WHERE id IN (
                        SELECT id FROM learning_materials
                        WHERE deleted_at IS NULL
                          AND is_active = true
                          AND thumbnail_status = 'failed'
                          AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                          AND updated_at < NOW() - INTERVAL '10 minutes'
                        LIMIT 2
                    )
                    "#,
                )
                .execute(pool),
            )
            .await;
            let _ = with_system_rls_bypass(
                sqlx::query(
                    r#"
                    UPDATE assignments
                    SET thumbnail_status = 'pending'
                    WHERE id IN (
                        SELECT id FROM assignments
                        WHERE deleted_at IS NULL
                          AND is_active = true
                          AND thumbnail_status = 'failed'
                          AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                          AND updated_at < NOW() - INTERVAL '10 minutes'
                        LIMIT 2
                    )
                    "#,
                )
                .execute(pool),
            )
            .await;
        }
    }
}

/// Drains all already-published historical materials and assignments cleanly on server boot
/// under an advisory lock so every existing module & assignment receives a 1x cached thumbnail.
pub async fn run_startup_thumbnail_catchup(pool: PgPool) {
    let Ok(mut lock_conn) = pool.acquire().await else {
        return;
    };
    let locked: bool = with_system_rls_bypass(
        sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock(8842009)")
            .fetch_one(&mut *lock_conn),
    )
    .await
    .unwrap_or(false);
    if !locked {
        return;
    }

    // Recover any interrupted 'processing', previously 'failed', or empty-URL rows back to 'pending'.
    let _ = with_system_rls_bypass(
        sqlx::query(
            "UPDATE learning_materials SET thumbnail_status = 'pending' WHERE deleted_at IS NULL AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '') AND thumbnail_status != 'pending'",
        )
        .execute(&pool),
    )
    .await;
    let _ = with_system_rls_bypass(
        sqlx::query(
            "UPDATE assignments SET thumbnail_status = 'pending' WHERE deleted_at IS NULL AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '') AND thumbnail_status != 'pending'",
        )
        .execute(&pool),
    )
    .await;

    let mut total_done = 0usize;
    let mut backoff_cycles = 0usize;
    // Bounded up to 360 iterations (~2000+ unique titles) with rate-limit bucket cooldowns.
    for _ in 0..360 {
        let (progressed, hit_rate_limit) =
            backfill_published_thumbnails_with_limits_inner(&pool, 6, 6).await;
        total_done += progressed;
        if hit_rate_limit || progressed == 0 {
            let remaining = count_remaining_pending_thumbnails(&pool).await;
            if remaining == 0 {
                break;
            }
            backoff_cycles += 1;
            if backoff_cycles >= 36 {
                tracing::warn!(
                    component = "thumbnail_catchup",
                    remaining,
                    total_done,
                    "Pausing startup thumbnail catch-up after extended rate-limit backoffs; 60s worker will resume"
                );
                break;
            }
            let sleep_secs = if backoff_cycles >= 3 { 45 } else { 25 };
            tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
        } else {
            backoff_cycles = 0;
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        }
    }

    let _ = with_system_rls_bypass(
        sqlx::query("SELECT pg_advisory_unlock(8842009)").execute(&mut *lock_conn),
    )
    .await;

    tracing::info!(
        component = "thumbnail_catchup",
        total_done,
        "Completed startup historical thumbnail catch-up"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_youtube_id_across_common_formats() {
        assert_eq!(
            extract_youtube_thumbnail(Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg".to_string())
        );
        assert_eq!(
            extract_youtube_thumbnail(Some("https://youtu.be/abcDEF12345?si=xyz")),
            Some("https://i.ytimg.com/vi/abcDEF12345/hqdefault.jpg".to_string())
        );
        assert_eq!(
            extract_youtube_thumbnail(Some("https://www.youtube.com/embed/M7lc1UVf-VE?autoplay=1")),
            Some("https://i.ytimg.com/vi/M7lc1UVf-VE/hqdefault.jpg".to_string())
        );
        assert_eq!(
            extract_youtube_thumbnail(Some("https://www.youtube.com/shorts/9bZkp7q19f0")),
            Some("https://i.ytimg.com/vi/9bZkp7q19f0/hqdefault.jpg".to_string())
        );
        assert_eq!(
            extract_youtube_thumbnail(Some("https://www.youtube.com/watch?feature=share&v=jNQXAC9IVRw&t=12")),
            Some("https://i.ytimg.com/vi/jNQXAC9IVRw/hqdefault.jpg".to_string())
        );
        assert_eq!(
            extract_youtube_thumbnail(Some("https://example.com/file.pdf")),
            None
        );
    }

    #[test]
    fn extracts_infographic_cover_without_regenerating() {
        let blocks_json = r#"[
            {"id":"b1","type":"TEXT","content":"Pendahuluan"},
            {"id":"ai-block-hero-cover-123","type":"IMAGE","content":"/uploads/ai_infografis_hero.png"},
            {"id":"ai-block-img-1-123","type":"IMAGE","content":"/uploads/ai_infografis_step1.png"}
        ]"#;
        assert_eq!(
            extract_infographic_cover_thumbnail(Some(blocks_json), None),
            Some("/uploads/ai_infografis_hero.png".to_string())
        );

        assert_eq!(
            extract_infographic_cover_thumbnail(None, Some("uploads/ai_infografis_direct.jpg")),
            Some("/uploads/ai_infografis_direct.jpg".to_string())
        );

        let wrapped_json = r#"{"title":"Ekonomi","blocks":[{"id":"i1","block_type":"IMAGE","content":"https://cdn.example.com/cover.webp"}]}"#;
        assert_eq!(
            extract_infographic_cover_thumbnail(Some(wrapped_json), None),
            Some("https://cdn.example.com/cover.webp".to_string())
        );

        let markdown = "# Judul\n\n![Cover Majalah](/uploads/mag_cover.jpg)\n\nTeks...";
        assert_eq!(
            extract_infographic_cover_thumbnail(Some(markdown), None),
            Some("/uploads/mag_cover.jpg".to_string())
        );
    }
}
