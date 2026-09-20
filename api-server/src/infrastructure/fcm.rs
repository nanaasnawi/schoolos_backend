use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Serialize)]
struct GoogleJwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    exp: i64,
    iat: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceAccountKey {
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub client_email: String,
    #[serde(default)]
    pub private_key: String,
}

fn load_service_account() -> Option<ServiceAccountKey> {
    // 1. Try FIREBASE_SERVICE_ACCOUNT environment variable (JSON string)
    if let Ok(json_str) = std::env::var("FIREBASE_SERVICE_ACCOUNT") {
        let trimmed = json_str.trim();
        if !trimmed.is_empty() {
            if let Ok(sa) = serde_json::from_str::<ServiceAccountKey>(trimmed) {
                if !sa.private_key.is_empty() {
                    return Some(sa);
                }
            }
        }
    }

    // 2. Try GOOGLE_APPLICATION_CREDENTIALS (file path or inline JSON)
    if let Ok(val) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        let trimmed = val.trim();
        if trimmed.starts_with('{') {
            if let Ok(sa) = serde_json::from_str::<ServiceAccountKey>(trimmed) {
                if !sa.private_key.is_empty() {
                    return Some(sa);
                }
            }
        } else if let Ok(content) = std::fs::read_to_string(trimmed) {
            if let Ok(sa) = serde_json::from_str::<ServiceAccountKey>(&content) {
                if !sa.private_key.is_empty() {
                    return Some(sa);
                }
            }
        }
    }

    // 3. Try FIREBASE_PRIVATE_KEY or FCM_PRIVATE_KEY or PRIVATE_KEY
    for env_key in &["FIREBASE_PRIVATE_KEY", "FCM_PRIVATE_KEY", "PRIVATE_KEY"] {
        if let Ok(val) = std::env::var(env_key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                // Check if user pasted the whole service account JSON
                if trimmed.starts_with('{') {
                    if let Ok(sa) = serde_json::from_str::<ServiceAccountKey>(trimmed) {
                        if !sa.private_key.is_empty() {
                            return Some(sa);
                        }
                    }
                }
                let private_key = trimmed.replace("\\n", "\n");
                let client_email = std::env::var("FIREBASE_CLIENT_EMAIL")
                    .or_else(|_| std::env::var("FCM_CLIENT_EMAIL"))
                    .unwrap_or_else(|_| "firebase-adminsdk-fbsvc@akselerasi-edu.iam.gserviceaccount.com".to_string());
                let project_id = std::env::var("FCM_PROJECT_ID")
                    .or_else(|_| std::env::var("FIREBASE_PROJECT_ID"))
                    .unwrap_or_else(|_| "akselerasi-edu".to_string());
                return Some(ServiceAccountKey {
                    project_id,
                    client_email,
                    private_key,
                });
            }
        }
    }

    // 4. Try local file paths
    let candidate_paths = [
        "firebase-service-account.json",
        "api-server/firebase-service-account.json",
        "../android/akselerasi-edu-firebase-adminsdk-fbsvc-42d4e6306a.json",
        "c:/Users/USER/Documents/School Os/android/akselerasi-edu-firebase-adminsdk-fbsvc-42d4e6306a.json",
    ];

    for path in &candidate_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(sa) = serde_json::from_str::<ServiceAccountKey>(&content) {
                if !sa.private_key.is_empty() {
                    return Some(sa);
                }
            }
        }
    }

    None
}

async fn get_access_token(client: &reqwest::Client) -> Result<(String, String), String> {
    // Check if we have a service account key
    if let Some(mut sa) = load_service_account() {
        if sa.project_id.is_empty() {
            sa.project_id = "akselerasi-edu".to_string();
        }
        let now = chrono::Utc::now().timestamp();
        let claims = GoogleJwtClaims {
            iss: &sa.client_email,
            scope: "https://www.googleapis.com/auth/firebase.messaging",
            aud: "https://oauth2.googleapis.com/token",
            exp: now + 3600,
            iat: now,
        };

        let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| format!("Invalid RSA private key: {}", e))?;

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let jwt = jsonwebtoken::encode(&header, &claims, &key)
            .map_err(|e| format!("Failed to encode JWT assertion: {}", e))?;

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ];

        let token_res = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to send OAuth2 JWT token request: {}", e))?;

        if !token_res.status().is_success() {
            let status = token_res.status();
            let err_text = token_res.text().await.unwrap_or_default();
            return Err(format!("OAuth2 token endpoint returned {}: {}", status, err_text));
        }

        let token_data = token_res
            .json::<GoogleTokenResponse>()
            .await
            .map_err(|e| format!("Failed to parse Google OAuth2 token response: {}", e))?;

        return Ok((token_data.access_token, sa.project_id));
    }

    // Fallback: OAuth2 refresh token flow
    let client_id = std::env::var("FCM_CLIENT_ID")
        .map_err(|_| "Neither Service Account nor FCM_CLIENT_ID configured".to_string())?;
    let client_secret = std::env::var("FCM_CLIENT_SECRET")
        .map_err(|_| "FCM_CLIENT_SECRET not set".to_string())?;
    let refresh_token = std::env::var("FCM_REFRESH_TOKEN")
        .map_err(|_| "FCM_REFRESH_TOKEN not set".to_string())?;
    let project_id = std::env::var("FCM_PROJECT_ID").unwrap_or_else(|_| "akselerasi-edu".to_string());

    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("refresh_token", refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];

    let token_res = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to request FCM OAuth2 token via refresh_token: {}", e))?;

    let token_data = token_res
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| format!("Failed to parse FCM OAuth2 token response: {}", e))?;

    Ok((token_data.access_token, project_id))
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
            FcmCategory::Announcement => "school_os_announcements_v4",
            _ => "school_os_learning_v2",
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
        let (access_token, project_id) = match get_access_token(&client).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("FCM authentication skipped or failed: {}", e);
                return;
            }
        };

        // DATA-ONLY High-Priority FCM Push — PENTING untuk idle/standby/Doze.
        // Payload `notification` sengaja DIHAPUS karena saat ada key `notification`,
        // Android menyerahkan render ke System Tray dan onMessageReceived() TIDAK
        // dipanggil saat app background/killed → helper kustom (dedup, wake, deep-link)
        // tidak jalan dan notifikasi sering hilang di HP idle.
        // Dengan data-only + android.priority=HIGH, FCM membangunkan aplikasi via
        // com.google.firebase.MESSAGING_EVENT walau Doze, lalu helper menampilkan
        // notifikasi PRIORITY_MAX + WakeLock sehingga muncul di lock screen.
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
                    "direct_boot_ok": true
                }
            }
        });

        match client
            .post(&fcm_url)
            .bearer_auth(access_token)
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

