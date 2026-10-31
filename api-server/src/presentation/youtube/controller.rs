use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::bootstrap::ApplicationContext;

#[derive(Debug, Deserialize)]
pub struct YoutubeSearchQuery {
    pub q: Option<String>,
    #[serde(alias = "maxResults")]
    pub max_results: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct YoutubeThumbnailItem {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct YoutubeThumbnails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<YoutubeThumbnailItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<YoutubeThumbnailItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<YoutubeThumbnailItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct YoutubeSnippet {
    pub title: String,
    pub description: String,
    #[serde(rename = "channelTitle")]
    pub channel_title: String,
    #[serde(rename = "publishedAt", skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnails: Option<YoutubeThumbnails>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct YoutubeVideoItem {
    pub id: String,
    #[serde(rename = "videoId")]
    pub video_id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "channelTitle")]
    pub channel_title: String,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    #[serde(rename = "publishedAt", skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<YoutubeSnippet>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YoutubeSearchSuccessResponse {
    pub success: bool,
    pub items: Vec<YoutubeVideoItem>,
    #[serde(rename = "totalResults")]
    pub total_results: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YoutubeSearchErrorResponse {
    pub success: bool,
    pub error: String,
    pub items: Vec<YoutubeVideoItem>,
}

pub fn youtube_routes() -> Router<ApplicationContext> {
    Router::new().route("/search", get(search_youtube_handler))
}

fn unescape_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

pub async fn search_youtube_handler(
    Query(params): Query<YoutubeSearchQuery>,
) -> Response {
    let query = match params.q.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(q) => q,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(YoutubeSearchErrorResponse {
                    success: false,
                    error: "Parameter pencarian 'q' wajib diisi".to_string(),
                    items: vec![],
                }),
            )
                .into_response();
        }
    };

    let api_key = match std::env::var("YOUTUBE_API_KEY")
        .or_else(|_| std::env::var("NEXT_PUBLIC_YOUTUBE_API_KEY"))
    {
        Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => {
            tracing::warn!("YOUTUBE_API_KEY is not configured in environment variables");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(YoutubeSearchErrorResponse {
                    success: false,
                    error: "YouTube API Key belum dikonfigurasi di environment server (YOUTUBE_API_KEY).".to_string(),
                    items: vec![],
                }),
            )
                .into_response();
        }
    };

    let max_results = params.max_results.unwrap_or(30).clamp(1, 50);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let url = "https://www.googleapis.com/youtube/v3/search";

    let resp = match client
        .get(url)
        .query(&[
            ("part", "snippet"),
            ("type", "video"),
            ("q", &query),
            ("maxResults", &max_results.to_string()),
            ("key", &api_key),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to contact Google YouTube API: {:?}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(YoutubeSearchErrorResponse {
                    success: false,
                    error: format!("Gagal menghubungi Google YouTube API: {e}"),
                    items: vec![],
                }),
            )
                .into_response();
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        tracing::error!("Google YouTube API returned {}: {}", status, err_body);
        let error_msg = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&err_body) {
            val["error"]["message"]
                .as_str()
                .unwrap_or("YouTube API error")
                .to_string()
        } else {
            format!("YouTube API HTTP {status}")
        };

        return (
            StatusCode::BAD_GATEWAY,
            Json(YoutubeSearchErrorResponse {
                success: false,
                error: format!("YouTube API error: {error_msg}"),
                items: vec![],
            }),
        )
            .into_response();
    }

    let raw_json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to parse Google YouTube API JSON response: {:?}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(YoutubeSearchErrorResponse {
                    success: false,
                    error: "Gagal memproses data respon dari YouTube API".to_string(),
                    items: vec![],
                }),
            )
                .into_response();
        }
    };

    let mut items: Vec<YoutubeVideoItem> = Vec::new();

    if let Some(raw_items) = raw_json["items"].as_array() {
        for item in raw_items {
            let video_id = item["id"]["videoId"]
                .as_str()
                .or_else(|| item["id"].as_str())
                .unwrap_or("")
                .to_string();

            if video_id.is_empty() {
                continue;
            }

            let snippet = &item["snippet"];
            let raw_title = snippet["title"].as_str().unwrap_or("").to_string();
            let raw_desc = snippet["description"].as_str().unwrap_or("").to_string();
            let raw_channel = snippet["channelTitle"].as_str().unwrap_or("").to_string();
            let published_at = snippet["publishedAt"].as_str().map(|s| s.to_string());

            let title = unescape_html(&raw_title);
            let description = unescape_html(&raw_desc);
            let channel_title = unescape_html(&raw_channel);

            let medium_thumb = snippet["thumbnails"]["medium"]["url"].as_str();
            let default_thumb = snippet["thumbnails"]["default"]["url"].as_str();
            let high_thumb = snippet["thumbnails"]["high"]["url"].as_str();

            let thumbnail_url = medium_thumb
                .or(default_thumb)
                .or(high_thumb)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("https://img.youtube.com/vi/{video_id}/mqdefault.jpg"));

            let thumbnails = YoutubeThumbnails {
                default: default_thumb.map(|u| YoutubeThumbnailItem {
                    url: u.to_string(),
                    width: None,
                    height: None,
                }),
                medium: medium_thumb.map(|u| YoutubeThumbnailItem {
                    url: u.to_string(),
                    width: None,
                    height: None,
                }),
                high: high_thumb.map(|u| YoutubeThumbnailItem {
                    url: u.to_string(),
                    width: None,
                    height: None,
                }),
            };

            let snippet_obj = YoutubeSnippet {
                title: title.clone(),
                description: description.clone(),
                channel_title: channel_title.clone(),
                published_at: published_at.clone(),
                thumbnails: Some(thumbnails),
            };

            items.push(YoutubeVideoItem {
                id: video_id.clone(),
                video_id,
                title,
                description,
                channel_title,
                thumbnail_url,
                published_at,
                snippet: Some(snippet_obj),
            });
        }
    }

    let total = items.len();

    (
        StatusCode::OK,
        Json(YoutubeSearchSuccessResponse {
            success: true,
            items,
            total_results: total,
        }),
    )
        .into_response()
}
