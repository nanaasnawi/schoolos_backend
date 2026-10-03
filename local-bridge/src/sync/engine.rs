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

        // 7. Siapkan Payload & Kirim ke School OS Cloud Hub dalam Batch
        let sync_endpoint = format!(
            "{}/api/v1/dapodik/agent/sync",
            cloud_url.trim_end_matches('/')
        );
        let http_client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        let batch_size = 50;
        let total_batches = if student_list.is_empty() {
            1
        } else {
            (student_list.len() + batch_size - 1) / batch_size
        };

        for batch_index in 0..total_batches {
            let start = batch_index * batch_size;
            let end = (start + batch_size).min(student_list.len());
            let chunk_students = if student_list.is_empty() {
                Vec::new()
            } else {
                student_list[start..end].to_vec()
            };

            // Profil Sekolah, GTK, dan Rombel dikirim bersama batch 0
            let (b_sekolah, b_gtk, b_rombel) = if batch_index == 0 {
                (
                    sekolah_val.clone(),
                    Some(gtk_list.clone()),
                    Some(rombel_list.clone()),
                )
            } else {
                (None, Some(vec![]), Some(vec![]))
            };

            let payload = AgentSyncPayload {
                dapodik_url: Some(dapodik_url.to_string()),
                npsn: Some(npsn.to_string()),
                bearer_token: Some(dapodik_token.to_string()),
                raw_sekolah: b_sekolah,
                raw_gtk: b_gtk,
                raw_rombel: b_rombel,
                raw_students: Some(chunk_students),
                synced_by: synced_by.map(|s| s.to_string()),
                batch_index: Some(batch_index),
                total_batches: Some(total_batches),
            };

            let mut req = http_client.post(&sync_endpoint).json(&payload);

            if !cloud_token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", cloud_token));
            }

            let resp = req.send().await.map_err(|e| {
                format!(
                    "Gagal mengirim data (batch {}/{}) ke School OS Cloud ({:?}): {}. Pastikan koneksi internet aktif.",
                    batch_index + 1, total_batches, sync_endpoint, e
                )
            })?;

            let status = resp.status();
            let resp_body = resp.text().await.unwrap_or_default();

            if !status.is_success() {
                return Err(format!("Server Cloud merespon status {}: {}", status, resp_body).into());
            }

            info!(
                "Batch {}/{} berhasil diterima Cloud ({} Siswa)",
                batch_index + 1,
                total_batches,
                end - start
            );
        }

        info!(
            "Sinkronisasi berhasil: {} GTK, {} Rombel, {} Siswa (Total diproses cloud: {})",
            total_gtk, total_rombel, total_students, total_students
        );

        Ok(SyncSummary {
            total_gtk,
            total_rombel,
            total_students,
            cloud_records_synced: total_students,
            status: "SUCCESS".to_string(),
            message: format!(
                "Berhasil menyinkronkan {} Siswa, {} GTK, dan {} Rombel ke Cloud School OS.",
                total_students, total_gtk, total_rombel
            ),
        })
    }
}
