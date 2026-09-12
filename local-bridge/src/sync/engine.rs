use crate::dapodik_acl::client::DapodikLocalClient;
use crate::dapodik_acl::models::AgentSyncPayload;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    pub total_gtk: usize,
    pub total_rombel: usize,
    pub total_students: usize,
    pub cloud_records_synced: usize,
    pub status: String,
    pub message: String,
}

pub struct SyncEngine;

impl SyncEngine {
    pub async fn perform_sync_params(
        cloud_url: &str,
        cloud_token: &str,
        dapodik_url: &str,
        npsn: &str,
        dapodik_token: &str,
        synced_by: Option<&str>,
    ) -> Result<SyncSummary, Box<dyn std::error::Error>> {
        info!("Memulai sinkronisasi on-demand dari Web Dashboard...");

        // 1. Inisialisasi Klien Dapodik Localhost
        let dapodik_client = DapodikLocalClient::new(
            dapodik_url.to_string(),
            dapodik_token.to_string(),
            npsn.to_string(),
        );

        // 2. Cek Koneksi ke Dapodik
        dapodik_client
            .test_connection()
            .await
            .map_err(|e| format!("Koneksi ke Dapodik lokal (port 5774) gagal: {}", e))?;

        // 3. Tarik Data Profil Sekolah
        let sekolah_val = match dapodik_client.fetch_sekolah().await {
            Ok(s) => Some(s),
            Err(e) => {
                warn!("Peringatan getSekolah: {}", e);
                None
            }
        };

        // 4. Tarik GTK (Guru & Tendik)
        let gtk_list = dapodik_client
            .fetch_gtk()
            .await
            .map_err(|e| format!("Gagal mengambil data GTK dari Dapodik: {}", e))?;

        // 5. Tarik Rombel & Pembelajaran (Mapel)
        let rombel_list = dapodik_client
            .fetch_rombel()
            .await
            .map_err(|e| format!("Gagal mengambil data Rombel dari Dapodik: {}", e))?;

        // 6. Tarik Peserta Didik
        let student_list = dapodik_client
            .fetch_peserta_didik()
            .await
            .map_err(|e| format!("Gagal mengambil data Siswa dari Dapodik: {}", e))?;

        let total_gtk = gtk_list.len();
        let total_rombel = rombel_list.len();
        let total_students = student_list.len();

        // 7. Siapkan Payload untuk Cloud Hub
        let payload = AgentSyncPayload {
            dapodik_url: Some(dapodik_url.to_string()),
            npsn: Some(npsn.to_string()),
            bearer_token: Some(dapodik_token.to_string()),
            raw_sekolah: sekolah_val,
            raw_gtk: Some(gtk_list),
            raw_rombel: Some(rombel_list),
            raw_students: Some(student_list),
            synced_by: synced_by.map(|s| s.to_string()),
        };

        // 8. Kirim ke School OS Cloud Hub
        let sync_endpoint = format!("{}/api/v1/dapodik/agent/sync", cloud_url.trim_end_matches('/'));
        let http_client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;

        let mut req = http_client.post(&sync_endpoint).json(&payload);

        if !cloud_token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", cloud_token));
        }

        let resp = req.send().await.map_err(|e| {
            format!(
                "Gagal mengirim data ke School OS Cloud ({:?}): {}. Pastikan koneksi internet aktif.",
                sync_endpoint, e
            )
        })?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!(
                "Server Cloud merespon status {}: {}",
                status, resp_body
            )
            .into());
        }

        // Hitung total records yang diproses
        let records_synced = match serde_json::from_str::<serde_json::Value>(&resp_body) {
            Ok(json_resp) => {
                if let Some(data_arr) = json_resp.get("data").and_then(|d| d.as_array()) {
                    data_arr.len()
                } else {
                    total_students
                }
            }
            Err(_) => total_students,
        };

        info!(
            "Sinkronisasi berhasil: {} GTK, {} Rombel, {} Siswa (Total diproses cloud: {})",
            total_gtk, total_rombel, total_students, records_synced
        );

        Ok(SyncSummary {
            total_gtk,
            total_rombel,
            total_students,
            cloud_records_synced: records_synced,
            status: "SUCCESS".to_string(),
            message: format!(
                "Berhasil menyinkronkan {} Siswa, {} GTK, dan {} Rombel ke Cloud School OS.",
                total_students, total_gtk, total_rombel
            ),
        })
    }
}
