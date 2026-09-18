use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

pub fn trigger_fcm_push_notification(title: String, content: String, category: String, reference_id: Uuid) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let client_id = match std::env::var("FCM_CLIENT_ID") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => {
                tracing::warn!("FCM_CLIENT_ID not set, skipping FCM push");
                return;
            }
        };
        let client_secret = match std::env::var("FCM_CLIENT_SECRET") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => {
                tracing::warn!("FCM_CLIENT_SECRET not set, skipping FCM push");
                return;
            }
        };
        let refresh_token = match std::env::var("FCM_REFRESH_TOKEN") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => {
                tracing::warn!("FCM_REFRESH_TOKEN not set, skipping FCM push");
                return;
            }
        };
        let project_id = std::env::var("FCM_PROJECT_ID").unwrap_or_else(|_| {
            "school-os-678a5".to_string()
        });

        // 1. Exchange refresh_token for Google access_token
        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];

        let token_res = match client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Failed to request FCM OAuth2 token: {}", e);
                return;
            }
        };

        let token_data = match token_res.json::<GoogleTokenResponse>().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to parse FCM OAuth2 token response: {}", e);
                return;
            }
        };

        // 2. Send High-Priority FCM Push Message with full Notification payload for Lock Screen & Standby display
        let fcm_url = format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", project_id);
        let payload = serde_json::json!({
            "message": {
                "topic": "school_announcements",
                "notification": {
                    "title": &title,
                    "body": &content
                },
                "data": {
                    "id": reference_id.to_string(),
                    "title": &title,
                    "body": &content,
                    "category": &category,
                    "navigate_to": "notifications",
                    "reference_type": "announcement",
                    "reference_id": reference_id.to_string()
                },
                "android": {
                    "priority": "high",
                    "ttl": "86400s",
                    "notification": {
                        "channel_id": "school_os_announcements_v3",
                        "notification_priority": "PRIORITY_MAX",
                        "visibility": "PUBLIC",
                        "default_sound": true,
                        "default_vibrate_timings": true,
                        "default_light_settings": true,
                        "icon": "ic_launcher",
                        "click_action": "OPEN_NOTIFICATIONS"
                    }
                }
            }
        });

        match client
            .post(&fcm_url)
            .bearer_auth(token_data.access_token)
            .json(&payload)
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                tracing::info!("FCM push notification sent successfully for reference {}", reference_id);
            }
            Ok(res) => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                tracing::warn!("FCM push notification responded with status {}: {}", status, text);
            }
            Err(e) => {
                tracing::warn!("Failed to send FCM push notification: {}", e);
            }
        }
    });
}
