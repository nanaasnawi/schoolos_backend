use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub cloud_url: String,
    pub cloud_token: String,
    pub dapodik_url: String,
    pub npsn: String,
    pub dapodik_token: String,
    pub sync_interval_mins: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            cloud_url: "https://schoolosbackend-production.up.railway.app".to_string(),
            cloud_token: String::new(),
            dapodik_url: "http://127.0.0.1:5774".to_string(),
            npsn: String::new(),
            dapodik_token: String::new(),
            sync_interval_mins: 60,
        }
    }
}

pub fn default_config_path() -> PathBuf {
    // 1. Check next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let next_to_exe = parent.join("schoolos-agent.json");
            if next_to_exe.exists() {
                return next_to_exe;
            }
        }
    }

    // 2. Check current working directory
    let cur = PathBuf::from("schoolos-agent.json");
    if cur.exists() {
        return cur;
    }

    // 3. Check Windows Downloads, Desktop, and Documents folders
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let downloads = PathBuf::from(&user_profile).join("Downloads").join("schoolos-agent.json");
        if downloads.exists() {
            println!("📂 Otomatis menemukan konfigurasi di folder Downloads: {:?}", downloads);
            return downloads;
        }

        let desktop = PathBuf::from(&user_profile).join("Desktop").join("schoolos-agent.json");
        if desktop.exists() {
            println!("📂 Otomatis menemukan konfigurasi di Desktop: {:?}", desktop);
            return desktop;
        }

        let documents = PathBuf::from(&user_profile).join("Documents").join("schoolos-agent.json");
        if documents.exists() {
            println!("📂 Otomatis menemukan konfigurasi di Documents: {:?}", documents);
            return documents;
        }
    }

    PathBuf::from("schoolos-agent.json")
}

pub fn load_config(path: Option<&str>) -> Result<AgentConfig, Box<dyn std::error::Error>> {
    let p = path.map(PathBuf::from).unwrap_or_else(default_config_path);
    if !p.exists() {
        return Err(format!("File konfigurasi {:?} tidak ditemukan.", p).into());
    }

    let contents = fs::read_to_string(&p)?;
    let cfg: AgentConfig = serde_json::from_str(&contents)?;
    Ok(cfg)
}

pub fn save_config(path: &Path, config: &AgentConfig) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn prompt_line(prompt: &str, default: Option<&str>) -> String {
    print!("{}", prompt);
    if let Some(def) = default {
        if !def.is_empty() {
            print!(" [{}]: ", def);
        } else {
            print!(": ");
        }
    } else {
        print!(": ");
    }
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn interactive_setup(target_path: &Path) -> Result<AgentConfig, Box<dyn std::error::Error>> {
    println!();
    println!("===============================================================");
    println!("   🏫 SETUP PENGATURAN SCHOOL OS BRIDGE AGENT (PORTABLE)      ");
    println!("===============================================================");
    println!("Konfigurasi ini akan disimpan di: {:?}", target_path);
    println!();

    let default_cloud = "https://schoolosbackend-production.up.railway.app";
    let default_dapodik = "http://127.0.0.1:5774";
    let default_interval = "60";

    let cloud_url = prompt_line("1. URL Server School OS Cloud", Some(default_cloud));
    let cloud_token = prompt_line(
        "2. Token Auth Operator School OS (dari Dashboard > Dapodik Hub)",
        None,
    );
    let dapodik_url = prompt_line("3. URL WebService Dapodik Lokal", Some(default_dapodik));
    let npsn = prompt_line("4. NPSN Sekolah", None);
    let dapodik_token = prompt_line("5. Token WebService Dapodik (dari Aplikasi Dapodik)", None);
    let interval_str = prompt_line("6. Interval Auto-Sync (Menit)", Some(default_interval));
    let sync_interval_mins = interval_str.parse::<u64>().unwrap_or(60);

    let config = AgentConfig {
        cloud_url: cloud_url.trim_end_matches('/').to_string(),
        cloud_token,
        dapodik_url: dapodik_url.trim_end_matches('/').to_string(),
        npsn,
        dapodik_token,
        sync_interval_mins,
    };

    save_config(target_path, &config)?;
    println!();
    println!("✅ Konfigurasi berhasil disimpan ke {:?}", target_path);
    println!("===============================================================");
    println!();

    Ok(config)
}
