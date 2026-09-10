use crate::config::AgentConfig;
use crate::dapodik_acl::client::DapodikLocalClient;
use crate::dapodik_acl::models::AgentSyncPayload;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct SyncSummary {
    pub total_gtk: usize,
    pub total_rombel: usize,
    pub total_students: usize,
    pub cloud_records_synced: usize,
    pub status: String,
}

pub struct SyncEngine;

impl SyncEngine {
    pub async fn perform_sync(
        config: &AgentConfig,
    ) -> Result<SyncSummary, Box<dyn std::error::Error>> {
        println!();
        println!("---------------------------------------------------------------");
        println!("🔄 Memulai Proses Sinkronisasi Dapodik -> Cloud School OS...");
        println!("---------------------------------------------------------------");

        // 1. Inisialisasi Klien Dapodik Localhost
        let dapodik_client = DapodikLocalClient::new(
            config.dapodik_url.clone(),
            config.dapodik_token.clone(),
            config.npsn.clone(),
        );

        // 2. Cek Koneksi ke Dapodik
        println!(
            "📡 [1/5] Memeriksa koneksi ke Dapodik di {} (NPSN: {})...",
            config.dapodik_url, config.npsn
        );
        dapodik_client
            .test_connection()
            .await
            .map_err(|e| format!("Koneksi Dapodik Gagal: {}", e))?;
        println!("   ✅ Terhubung ke Dapodik Localhost!");

        // 3. Tarik Data Profil Sekolah
        println!("🏫 [2/5] Mengambil profil sekolah...");
        let sekolah_val = match dapodik_client.fetch_sekolah().await {
            Ok(s) => Some(s),
            Err(e) => {
                println!("   ⚠️  Peringatan getSekolah: {}", e);
                None
            }
        };

        // 4. Tarik GTK (Guru & Tendik)
        println!("👨‍🏫 [3/5] Mengambil data GTK (Guru & Tenaga Kependidikan)...");
        let gtk_list = dapodik_client
            .fetch_gtk()
            .await
            .map_err(|e| format!("Gagal mengambil data GTK: {}", e))?;
        println!("   ✅ Ditemukan {} GTK.", gtk_list.len());

        // 5. Tarik Rombel & Pembelajaran (Mapel)
        println!("📚 [4/5] Mengambil data Rombongan Belajar & Pembelajaran...");
        let rombel_list = dapodik_client
            .fetch_rombel()
            .await
            .map_err(|e| format!("Gagal mengambil data Rombel: {}", e))?;
        println!("   ✅ Ditemukan {} Rombongan Belajar.", rombel_list.len());

        // 6. Tarik Peserta Didik
        println!("🎓 [5/5] Mengambil data Peserta Didik (Siswa)...");
        let student_list = dapodik_client
            .fetch_peserta_didik()
            .await
            .map_err(|e| format!("Gagal mengambil data Siswa: {}", e))?;
        println!("   ✅ Ditemukan {} Peserta Didik.", student_list.len());

        let total_gtk = gtk_list.len();
        let total_rombel = rombel_list.len();
        let total_students = student_list.len();

        // 7. Siapkan Payload untuk Cloud Hub
        let payload = AgentSyncPayload {
            dapodik_url: Some(config.dapodik_url.clone()),
            npsn: Some(config.npsn.clone()),
            bearer_token: Some(config.dapodik_token.clone()),
            raw_sekolah: sekolah_val,
            raw_gtk: Some(gtk_list),
            raw_rombel: Some(rombel_list),
            raw_students: Some(student_list),
        };

        // 8. Kirim ke School OS Cloud Hub
        let sync_endpoint = format!("{}/api/v1/dapodik/agent/sync", config.cloud_url);
        println!(
            "☁️  Mengirim {} entitas ke Cloud School OS ({}) ...",
            total_gtk + total_rombel + total_students,
            sync_endpoint
        );

        let http_client = Client::builder()
            .timeout(Duration::from_secs(180)) // Ingestion could take up to 3 mins for large schools
            .build()?;

        let mut req = http_client.post(&sync_endpoint).json(&payload);

        if !config.cloud_token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", config.cloud_token));
        }

        let resp = req.send().await.map_err(|e| {
            format!(
                "Gagal menghubungi School OS Cloud di {}: {}. Mohon cek koneksi internet Anda.",
                sync_endpoint, e
            )
        })?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!(
                "Server Cloud menolak sinkronisasi (HTTP {}): {}",
                status, resp_body
            )
            .into());
        }

        // Parse response to count synced records
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

        println!("---------------------------------------------------------------");
        println!("🎉 SINKRONISASI BERHASIL!");
        println!("   - GTK: {} terdata", total_gtk);
        println!("   - Rombel: {} kelas", total_rombel);
        println!("   - Siswa: {} murid", total_students);
        println!(
            "   - Total Catatan Diproses Cloud: {} entitas",
            records_synced
        );
        println!("   - Database PostgreSQL Cloud sudah mutakhir!");
        println!("---------------------------------------------------------------");

        Ok(SyncSummary {
            total_gtk,
            total_rombel,
            total_students,
            cloud_records_synced: records_synced,
            status: "SUCCESS".to_string(),
        })
    }

    pub fn start_daemon(config: AgentConfig) {
        info!("Starting background daemon loop...");

        tokio::spawn(async move {
            let interval = if config.sync_interval_mins == 0 {
                60
            } else {
                config.sync_interval_mins
            };

            loop {
                info!("Waiting {} minutes for next scheduled sync...", interval);
                sleep(Duration::from_secs(interval * 60)).await;

                info!("Executing scheduled background sync...");
                match Self::perform_sync(&config).await {
                    Ok(summary) => {
                        info!(
                            "Scheduled sync completed: {} records synced.",
                            summary.cloud_records_synced
                        );
                    }
                    Err(e) => {
                        error!("Scheduled sync encountered an error: {}", e);
                    }
                }
            }
        });
    }
}
