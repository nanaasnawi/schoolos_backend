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

/// Extracts a YouTube video ID from watch, short, embed, shorts, live, or youtu.be URLs.
pub fn extract_youtube_video_id(raw_url: &str) -> Option<String> {
    let url = raw_url.trim();
    if url.is_empty() {
        return None;
    }
    let lower = url.to_ascii_lowercase();

    // 1. youtu.be/<id>
    if let Some(pos) = lower.find("youtu.be/") {
        let rest = &url[pos + "youtu.be/".len()..];
        let id = rest.split(['?', '&', '#', '/']).next().unwrap_or("").trim();
        if is_valid_youtube_id(id) {
            return Some(id.to_string());
        }
    }

    // 2. youtube.com/watch?v=<id> or &v=<id>
    if lower.contains("youtube.com") || lower.contains("youtube-nocookie.com") {
        if let Some(query_pos) = url.find('?') {
            let query = &url[query_pos + 1..];
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k.eq_ignore_ascii_case("v") {
                        let id = v.split('#').next().unwrap_or("").trim();
                        if is_valid_youtube_id(id) {
                            return Some(id.to_string());
                        }
                    }
                }
            }
        }

        // 3. /embed/<id>, /shorts/<id>, /live/<id>, /v/<id>
        for marker in ["/embed/", "/shorts/", "/live/", "/v/"] {
            if let Some(pos) = lower.find(marker) {
                let rest = &url[pos + marker.len()..];
                let id = rest.split(['?', '&', '#', '/']).next().unwrap_or("").trim();
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

fn looks_like_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let without_query = lower.split(['?', '#']).next().unwrap_or(&lower);
    without_query.starts_with("/uploads/")
        || without_query.ends_with(".png")
        || without_query.ends_with(".jpg")
        || without_query.ends_with(".jpeg")
        || without_query.ends_with(".webp")
        || without_query.ends_with(".gif")
        || without_query.ends_with(".svg")
}

/// Extracts the magazine cover / primary hero image from an infographic material
/// without invoking FLUX AI again.
pub fn extract_infographic_cover_thumbnail(
    description: Option<&str>,
    external_url: Option<&str>,
) -> Option<String> {
    if let Some(ext) = external_url.and_then(normalize_image_url) {
        if looks_like_image_url(&ext) && !ext.to_ascii_lowercase().ends_with(".pdf") {
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
                if let Some(s) = obj.get(key).and_then(|v| v.as_str()).and_then(normalize_image_url) {
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
        return Some(md_url);
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

/// Resolves deterministic zero-AI thumbnails for VIDEO, INFOGRAPHIC, and IMAGE materials.
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
        "VIDEO" => extract_youtube_thumbnail(external_url),
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

    // 1. Instant zero-token resolution for Video and Infografis (also keeps updated links/blocks in sync).
    if let Some(deterministic_url) = resolve_deterministic_material_thumbnail(
        &material_type,
        external_url.as_deref(),
        description.as_deref(),
        source_type.as_deref(),
        library_book_id,
    ) {
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
        let _ = with_system_rls_bypass(
            sqlx::query(
                r#"
                UPDATE learning_materials
                SET thumbnail_url = $1,
                    thumbnail_status = 'ready'
                WHERE id = $2
                  AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                "#,
            )
            .bind(&reused)
            .bind(material_id)
            .execute(&pool),
        )
        .await;
        return Some(reused);
    }

    // Only ARTICLE and PDF (Buku & PDF) trigger 1x FLUX AI on publish.
    if fmt != "ARTICLE" && fmt != "PDF" {
        return None;
    }
    if !cloudflare_ai::is_configured() {
        if let Some(fallback_cover) = library_cover_url {
            let _ = with_system_rls_bypass(
                sqlx::query(
                    "UPDATE learning_materials SET thumbnail_url = $1, thumbnail_status = 'ready' WHERE id = $2 AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')",
                )
                .bind(&fallback_cover)
                .bind(material_id)
                .execute(&pool),
            )
            .await;
            return Some(fallback_cover);
        }
        return None;
    }

    // Atomic compare-and-set claim guarantees FLUX runs AT MOST ONCE per material.
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
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()
    .is_some();

    if !claimed {
        return None;
    }

    let snippet = summarize_context_for_prompt(description.as_deref());
    tokio::spawn(async move {
        let res = with_system_rls_bypass(cloudflare_ai::generate_module_thumbnail_image(
            &pool,
            &fmt,
            &title,
            &subject_name,
            &class_name,
            snippet.as_deref(),
        ))
        .await;

        match res {
            Ok(url) => {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        r#"
                        UPDATE learning_materials
                        SET thumbnail_url = $1,
                            thumbnail_status = 'ready'
                        WHERE id = $2
                          AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                        "#,
                    )
                    .bind(&url)
                    .bind(material_id)
                    .execute(&pool),
                )
                .await;
            }
            Err(cloudflare_ai::ThumbnailGenError::NotConfigured) => {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        "UPDATE learning_materials SET thumbnail_status = 'pending' WHERE id = $1 AND thumbnail_status = 'processing'",
                    )
                    .bind(material_id)
                    .execute(&pool),
                )
                .await;
            }
            Err(cloudflare_ai::ThumbnailGenError::Failed(err)) => {
                tracing::warn!(
                    material_id = %material_id,
                    "FLUX thumbnail generation failed cleanly: {err}"
                );
                if let Some(fallback_cover) = library_cover_url {
                    let _ = with_system_rls_bypass(
                        sqlx::query(
                            "UPDATE learning_materials SET thumbnail_url = $1, thumbnail_status = 'ready' WHERE id = $2",
                        )
                        .bind(&fallback_cover)
                        .bind(material_id)
                        .execute(&pool),
                    )
                    .await;
                } else {
                    let _ = with_system_rls_bypass(
                        sqlx::query(
                            "UPDATE learning_materials SET thumbnail_status = 'failed' WHERE id = $1 AND thumbnail_status = 'processing'",
                        )
                        .bind(material_id)
                        .execute(&pool),
                    )
                    .await;
                }
            }
        }
    });

    None
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
            || release_at.is_some_and(|r| r <= chrono::Utc::now()));
    if !is_live || thumbnail_status != "pending" {
        return None;
    }

    // Reuse sibling assignment thumbnail if published with the exact same title across multi-rombel.
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
        let _ = with_system_rls_bypass(
            sqlx::query(
                r#"
                UPDATE assignments
                SET thumbnail_url = $1,
                    thumbnail_status = 'ready'
                WHERE id = $2
                  AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                "#,
            )
            .bind(&reused)
            .bind(assignment_id)
            .execute(&pool),
        )
        .await;
        return Some(reused);
    }

    if !cloudflare_ai::is_configured() {
        return None;
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
        .fetch_optional(&pool),
    )
    .await
    .ok()
    .flatten()
    .is_some();

    if !claimed {
        return None;
    }

    let context = summarize_context_for_prompt(description.as_deref().or(instructions.as_deref()));
    tokio::spawn(async move {
        let res = with_system_rls_bypass(cloudflare_ai::generate_module_thumbnail_image(
            &pool,
            "ASSIGNMENT",
            &title,
            &subject_name,
            &class_name,
            context.as_deref(),
        ))
        .await;

        match res {
            Ok(url) => {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        r#"
                        UPDATE assignments
                        SET thumbnail_url = $1,
                            thumbnail_status = 'ready'
                        WHERE id = $2
                          AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
                        "#,
                    )
                    .bind(&url)
                    .bind(assignment_id)
                    .execute(&pool),
                )
                .await;
            }
            Err(cloudflare_ai::ThumbnailGenError::NotConfigured) => {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        "UPDATE assignments SET thumbnail_status = 'pending' WHERE id = $1 AND thumbnail_status = 'processing'",
                    )
                    .bind(assignment_id)
                    .execute(&pool),
                )
                .await;
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
                    .execute(&pool),
                )
                .await;
            }
        }
    });

    None
}

/// Runs once per worker cycle for already-published items that still have `thumbnail_status = 'pending'`.
/// Uses atomic `'pending' -> 'processing'` claims so every item can invoke FLUX at most once ever.
pub async fn backfill_published_thumbnails_batch(pool: &PgPool) {
    // 1. Zero-token deterministic backfill for VIDEO & INFOGRAPHIC materials
    if let Ok(mat_rows) = with_system_rls_bypass(
        sqlx::query(
            r#"
            SELECT id, material_type, description, source_type, library_book_id,
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
                  LOWER(COALESCE(material_type, '')) IN ('video', 'image', 'infographic')
                  OR LOWER(COALESCE(external_url, '')) LIKE '%youtube%'
                  OR LOWER(COALESCE(external_url, '')) LIKE '%youtu.be%'
                  OR LTRIM(COALESCE(description, '')) LIKE '[%'
              )
            ORDER BY updated_at DESC
            LIMIT 40
            "#,
        )
        .fetch_all(pool),
    )
    .await
    {
        for r in mat_rows {
            let id: Uuid = r.get("id");
            let m_type: String = r.get("material_type");
            let desc: Option<String> = r.try_get("description").ok().flatten();
            let src_type: Option<String> = r.try_get("source_type").ok().flatten();
            let lib_id: Option<Uuid> = r.try_get("library_book_id").ok().flatten();
            let ext_url: Option<String> = r.try_get("resolved_external_url").ok().flatten();

            if let Some(url) = resolve_deterministic_material_thumbnail(
                &m_type,
                ext_url.as_deref(),
                desc.as_deref(),
                src_type.as_deref(),
                lib_id,
            ) {
                let _ = with_system_rls_bypass(
                    sqlx::query(
                        "UPDATE learning_materials SET thumbnail_url = $1, thumbnail_status = 'ready' WHERE id = $2",
                    )
                    .bind(&url)
                    .bind(id)
                    .execute(pool),
                )
                .await;
            }
        }
    }

    if !cloudflare_ai::is_configured() {
        return;
    }

    // 2. Bounded batch (max 3 materials + max 3 assignments per tick) for already-published
    // Artikel / Buku & PDF / Assignments so existing rows populate cleanly without quota spikes.
    if let Ok(mat_ids) = with_system_rls_bypass(
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM learning_materials
            WHERE deleted_at IS NULL
              AND is_active = true
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (release_at IS NULL OR release_at <= NOW())
              AND (visibility IN ('published', 'active') OR (release_at IS NOT NULL AND release_at <= NOW()))
            ORDER BY updated_at DESC
            LIMIT 3
            "#,
        )
        .fetch_all(pool),
    )
    .await
    {
        for mid in mat_ids {
            let _ = ensure_material_thumbnail_on_publish(pool.clone(), mid).await;
        }
    }

    if let Ok(asg_ids) = with_system_rls_bypass(
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM assignments
            WHERE deleted_at IS NULL
              AND is_active = true
              AND thumbnail_status = 'pending'
              AND (thumbnail_url IS NULL OR BTRIM(thumbnail_url) = '')
              AND (release_at IS NULL OR release_at <= NOW())
              AND (status IN ('published', 'active', 'closed') OR (release_at IS NOT NULL AND release_at <= NOW()))
            ORDER BY updated_at DESC
            LIMIT 3
            "#,
        )
        .fetch_all(pool),
    )
    .await
    {
        for aid in asg_ids {
            let _ = ensure_assignment_thumbnail_on_publish(pool.clone(), aid).await;
        }
    }
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
