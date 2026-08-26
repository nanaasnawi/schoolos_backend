use super::adapter::DapodikAdapter;
use super::models::DapodikWsResponse;
use async_trait::async_trait;
use reqwest::{Client, header};
use tracing::{info, error};

pub struct DapodikWebServiceAdapter {
    client: Client,
    base_url: String,
    npsn: String,
}

impl DapodikWebServiceAdapter {
    pub fn new(base_url: String, token: String, npsn: String) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        
        let auth_val = match header::HeaderValue::from_str(&format!("Bearer {}", token)) {
            Ok(v) => v,
            Err(_) => return Err("Invalid token format".to_string()),
        };
        
        headers.insert(header::AUTHORIZATION, auth_val);
        
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            client,
            base_url,
            npsn,
        })
    }
}

#[async_trait]
impl DapodikAdapter for DapodikWebServiceAdapter {
    async fn connect_and_fingerprint(&self) -> Result<String, Box<dyn std::error::Error>> {
        // Since we are using the Web Service, our fingerprint is basically the API accessibility test
        info!("Testing Web Service connection to {}", self.base_url);
        
        let url = format!("{}/WebService/getSekolah?npsn={}", self.base_url, self.npsn);
        let res = self.client.get(&url).send().await?;
        
        if res.status().is_success() {
            Ok("dapodik_api_v2027_compatible".to_string())
        } else {
            let status = res.status();
            let body = res.text().await?;
            error!("Dapodik API Error ({}): {}", status, body);
            Err(format!("API Error: {}", status).into())
        }
    }

    async fn get_students(
        &self, 
        _cursor: i64, 
        _limit: i32
    ) -> Result<Vec<crate::domain::student::StudentSyncRecord>, Box<dyn std::error::Error>> {
        let url = format!("{}/WebService/getPesertaDidik?npsn={}", self.base_url, self.npsn);
        
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(format!("Failed to fetch students: {}", res.status()).into());
        }

        let body: DapodikWsResponse = res.json().await?;
        info!("Fetched {} students from Dapodik Web Service", body.rows.len());

        let sync_records = body.rows.into_iter()
            .map(super::mapper::map_ws_student_to_sync_record)
            .collect();

        Ok(sync_records)
    }

    async fn get_teachers(
        &self, 
        _cursor: i64, 
        _limit: i32
    ) -> Result<Vec<crate::domain::teacher::TeacherSyncRecord>, Box<dyn std::error::Error>> {
        // Mock implementation
        info!("Fetching teachers from Dapodik Web Service (Mock)");
        Ok(vec![])
    }

    async fn get_classes(
        &self, 
        _cursor: i64, 
        _limit: i32
    ) -> Result<Vec<crate::domain::class::ClassSyncRecord>, Box<dyn std::error::Error>> {
        // Mock implementation
        info!("Fetching classes from Dapodik Web Service (Mock)");
        Ok(vec![])
    }
}
