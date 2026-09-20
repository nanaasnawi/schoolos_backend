use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

/// Kategori notifikasi → dipakai untuk click-action, channel ringan, dan deep-link Android.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FcmCategory {
    Announcement,
    Material,
    Assignment,
    Quiz,
    Grade,
    Session,
    Reminder,
}

impl FcmCategory {
    fn as_str(&self) -> &'static str {
        match self {
            FcmCategory::Announcement => "ANNOUNCEMENT",
            FcmCategory::Material => "LEARNING_MATERIAL",
            FcmCategory::Assignment => "ASSIGNMENT",
            FcmCategory::Quiz => "QUIZ",
            FcmCategory::Grade => "GRADE_UPDATE",
            FcmCategory::Session => "SESSION_STARTED",
            FcmCategory::Reminder => "SMART_REMINDER",
        }
    }

    fn channel_id(&self) -> &'static str {
        match self {
            FcmCategory::Announcement => "school_os_announcements_v3",
            _ => "school_os_learning_v1",
        }
    }

    fn click_action(&self) -> &'static str {
        match self {
            FcmCategory::Announcement => "OPEN_NOTIFICATIONS",
            FcmCategory::Material => "OPEN_MATERIALS",
            FcmCategory::Assignment => "OPEN_ASSIGNMENTS",
            FcmCategory::Quiz => "OPEN_QUIZZES",
            FcmCategory::Grade => "OPEN_GRADES",
            FcmCategory::Session => "OPEN_SESSIONS",
            FcmCategory::Reminder => "OPEN_SCHEDULE",
        }
    }

    fn navigate_to(&self) -> &'static str {
        match self {
            FcmCategory::Announcement => "notifications",
            FcmCategory::Material => "materials",
            FcmCategory::Assignment => "assignments",
            FcmCategory::Quiz => "quizzes",
            FcmCategory::Grade => "grades",
            FcmCategory::Session => "sessions",
            FcmCategory::Reminder => "schedule",
        }
    }
}

fn parse_category(raw: &str) -> FcmCategory {
    let r = raw.to_lowercase();
    if r.contains("materi") || r.contains("material") || r.contains("pembelajaran") {
        FcmCategory::Material
    } else if r.contains("tugas") || r.contains("assignment") {
        FcmCategory::Assignment
    } else if r.contains("kuis") || r.contains("quiz") || r.contains("cbt") || r.contains("ujian") {
        FcmCategory::Quiz
    } else if r.contains("nilai") || r.contains("grade") || r.contains("rapor") || r.contains("rapot") {
        FcmCategory::Grade
    } else if r.contains("jadwal") || r.contains("sesi") || r.contains("session") || r.contains("kelas dimulai") || r.contains("mengajar") {
        FcmCategory::Session
    } else if r.contains("reminder") || r.contains("pengingat") {
        FcmCategory::Reminder
    } else {
        FcmCategory::Announcement
    }
}

pub fn trigger_fcm_push_notification(title: String, content: String, category: String, reference_id: Uuid) {
    let cat = parse_category(&category);
    trigger_fcm_push_categorized(title, content, cat, reference_id);
}

/// Varian eksplisit agar setiap tipe event (tugas/kuis/nilai/sesi) dapat channel & deep-link sendiri.
pub fn trigger_fcm_push_categorized(title: String, content: String, category: FcmCategory, reference_id: Uuid) {
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

        // 2. DATA-ONLY High-Priority FCM Push — PENTING untuk idle/standby/Doze.
        //    Payload `notification` sengaja DIHAPUS karena saat ada key `notification`,
        //    Android menyerahkan render ke System Tray dan onMessageReceived() TIDAK
        //    dipanggil saat app background/killed → helper kustom (dedup, wake, deep-link)
        //    tidak jalan dan notifikasi sering hilang di HP idle.
        //    Dengan data-only + android.priority=HIGH, FCM membangunkan aplikasi via
        //    com.google.firebase.MESSAGING_EVENT walau Doze, lalu helper menampilkan
        //    notifikasi PRIORITY_MAX + WakeLock sehingga muncul di lock screen.
        let fcm_url = format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", project_id);
        let channel_id = category.channel_id();
        let category_str = category.as_str();
        let click_action = category.click_action();
        let navigate_to = category.navigate_to();
        let payload = serde_json::json!({
            "message": {
                "topic": "school_announcements",
                "data": {
                    "id": reference_id.to_string(),
                    "title": &title,
                    "body": &content,
                    "content": &content,
                    "category": category_str,
                    "reference_type": category_str.to_lowercase(),
                    "reference_id": reference_id.to_string(),
                    "channel_id": channel_id,
                    "click_action": click_action,
                    "navigate_to": navigate_to
                },
                "android": {
                    "priority": "HIGH",
                    "ttl": "86400s",
                    "direct_boot_ok": true,
                    "notification": {
                        "channel_id": channel_id,
                        "notification_priority": "PRIORITY_MAX",
                        "visibility": "PUBLIC",
                        "default_sound": true,
                        "default_vibrate_timings": true,
                        "default_light_settings": true,
                        "icon": "ic_launcher",
                        "click_action": click_action
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
