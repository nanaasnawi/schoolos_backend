use super::models::{DapodikRawGtk, DapodikRawRombel, DapodikRawStudent};
use reqwest::Client;
use std::time::Duration;

pub struct DapodikLocalClient {
    client: Client,
    base_url: String,
    token: String,
    npsn: String,
}

impl DapodikLocalClient {
    pub fn new(base_url: String, token: String, npsn: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            npsn,
        }
    }

    pub async fn test_connection(&self) -> Result<String, String> {
        let url = format!("{}/WebService/getSekolah?npsn={}", self.base_url, self.npsn);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| {
                format!(
                    "Gagal menghubungi Dapodik di {}: {}. Pastikan Dapodik aktif di laptop ini.",
                    self.base_url, e
                )
            })?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            if body.contains("Access denied") {
                return Err(
                    "Akses WebService Dapodik Ditolak (Access Denied). Pastikan 'IP Pengakses' di Pengaturan Web Service Dapodik berisi '127.0.0.1' dan Token sudah sesuai."
                        .to_string(),
                );
            }
            return Err(format!(
                "Dapodik WebService merespon HTTP status {}: {}",
                status, body
            ));
        }

        Ok(body)
    }

    pub async fn fetch_sekolah(&self) -> Result<serde_json::Value, String> {
        let url = format!("{}/WebService/getSekolah?npsn={}", self.base_url, self.npsn);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Error getSekolah: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("getSekolah status: {}", resp.status()));
        }

        let val = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Format JSON getSekolah tidak valid: {}", e))?;

        Ok(val)
    }

    pub async fn fetch_gtk(&self) -> Result<Vec<DapodikRawGtk>, String> {
        let url = format!(
            "{}/WebService/getGtk?npsn={}&limit=5000",
            self.base_url, self.npsn
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Error getGtk: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("getGtk status: {}", resp.status()));
        }

        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON getGtk error: {}", e))?;

        let items: Vec<DapodikRawGtk> = if val.is_array() {
            serde_json::from_value(val).unwrap_or_default()
        } else if val.is_object() && val.get("rows").is_some() {
            serde_json::from_value(val["rows"].clone()).unwrap_or_default()
        } else {
            vec![]
        };

        Ok(items)
    }

    pub async fn fetch_rombel(&self) -> Result<Vec<DapodikRawRombel>, String> {
        let url = format!(
            "{}/WebService/getRombonganBelajar?npsn={}&limit=5000",
            self.base_url, self.npsn
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Error getRombel: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("getRombel status: {}", resp.status()));
        }

        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON getRombel error: {}", e))?;

        let items: Vec<DapodikRawRombel> = if val.is_array() {
            serde_json::from_value(val).unwrap_or_default()
        } else if val.is_object() && val.get("rows").is_some() {
            serde_json::from_value(val["rows"].clone()).unwrap_or_default()
        } else {
            vec![]
        };

        Ok(items)
    }

    pub async fn fetch_peserta_didik(&self) -> Result<Vec<DapodikRawStudent>, String> {
        let url = format!(
            "{}/WebService/getPesertaDidik?npsn={}&limit=5000",
            self.base_url, self.npsn
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("Error getPesertaDidik: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("getPesertaDidik status: {}", resp.status()));
        }

        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON getPesertaDidik error: {}", e))?;

        let items: Vec<DapodikRawStudent> = if val.is_array() {
            serde_json::from_value(val).unwrap_or_default()
        } else if val.is_object() && val.get("rows").is_some() {
            serde_json::from_value(val["rows"].clone()).unwrap_or_default()
        } else {
            vec![]
        };

        Ok(items)
    }
}
