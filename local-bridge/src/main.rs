pub mod auth;
pub mod config;
pub mod dapodik_acl;
pub mod domain;
pub mod store;
pub mod sync;

use config::{default_config_path, interactive_setup, load_config, AgentConfig};
use std::env;
use sync::engine::SyncEngine;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn print_banner() {
    println!();
    println!("===============================================================");
    println!("        🏫 SCHOOL OS BRIDGE AGENT (Windows Portable)          ");
    println!("   Sinkronisasi Otomatis Dapodik Localhost ke Cloud School OS  ");
    println!("===============================================================");
    println!("  • Tidak perlu unduh prefill .prf berulang-ulang             ");
    println!("  • Bebas firewall karena jalan langsung di PC Dapodik        ");
    println!("  • IP Pengakses WebService Dapodik tetap: 127.0.0.1           ");
    println!("===============================================================");
    println!();
}

fn print_help() {
    println!("Penggunaan: schoolos-sync [PILIHAN]");
    println!();
    println!("Pilihan:");
    println!("  --sync-now        Jalankan sinkronisasi sekali lalu selesai (cocok untuk Task Scheduler)");
    println!("  --setup           Jalankan wizard interaktif untuk memasukkan/mengubah konfigurasi");
    println!("  --test            Uji koneksi ke Dapodik lokal dan Cloud tanpa sinkronisasi");
    println!("  --config <path>   Gunakan file konfigurasi tertentu");
    println!("  --help, -h        Tampilkan panduan ini");
    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inisialisasi tracing logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    print_banner();

    let args: Vec<String> = env::args().collect();
    let mut custom_config_path: Option<String> = None;
    let mut sync_now_only = false;
    let mut run_setup = false;
    let mut test_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--sync-now" => sync_now_only = true,
            "--setup" => run_setup = true,
            "--test" => test_only = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--config" => {
                if i + 1 < args.len() {
                    custom_config_path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let target_cfg_path = custom_config_path
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_config_path);

    // 1. Muat atau buat konfigurasi
    let config: AgentConfig = if run_setup || !target_cfg_path.exists() {
        if !target_cfg_path.exists() && !run_setup {
            println!(
                "ℹ️  File konfigurasi {:?} belum ditemukan.",
                target_cfg_path
            );
            println!("   Memulai wizard pengaturan pertama kali...\n");
        }
        interactive_setup(&target_cfg_path)?
    } else {
        match load_config(custom_config_path.as_deref()) {
            Ok(cfg) => {
                println!("📂 Menggunakan konfigurasi dari: {:?}", target_cfg_path);
                cfg
            }
            Err(e) => {
                println!("⚠️  Gagal membaca konfigurasi: {}", e);
                println!("   Menjalankan wizard pengaturan ulang...\n");
                interactive_setup(&target_cfg_path)?
            }
        }
    };

    println!("   • Cloud URL:   {}", config.cloud_url);
    println!("   • Dapodik:     {} (NPSN: {})", config.dapodik_url, config.npsn);
    println!("   • Auto-Sync:   Tiap {} menit", config.sync_interval_mins);
    println!();

    // Mode Test
    if test_only {
        println!("🧪 Menguji koneksi Dapodik lokal...");
        let dapodik_client = dapodik_acl::client::DapodikLocalClient::new(
            config.dapodik_url.clone(),
            config.dapodik_token.clone(),
            config.npsn.clone(),
        );
        match dapodik_client.test_connection().await {
            Ok(_) => println!("   ✅ Koneksi Dapodik Localhost BERHASIL!"),
            Err(e) => println!("   ❌ Koneksi Dapodik GAGAL: {}", e),
        }
        return Ok(());
    }

    // 2. Jalankan sinkronisasi pertama
    match SyncEngine::perform_sync(&config).await {
        Ok(_) => {
            if sync_now_only {
                println!("✅ Sinkronisasi selesai. Program berakhir.");
                return Ok(());
            }
        }
        Err(e) => {
            eprintln!("❌ Sinkronisasi gagal: {}", e);
            if sync_now_only {
                std::process::exit(1);
            }
        }
    }

    // 3. Jika mode background / daemon aktif
    if config.sync_interval_mins > 0 {
        SyncEngine::start_daemon(config.clone());
        println!();
        println!("💡 Agent sekarang berjalan di latar belakang (background).");
        println!(
            "   Sinkronisasi otomatis akan dieksekusi setiap {} menit.",
            config.sync_interval_mins
        );
        println!("   Tekan Ctrl+C di terminal ini untuk menghentikan agent.");
        println!();

        tokio::signal::ctrl_c().await?;
        println!();
        info!("Menghentikan School OS Bridge Agent...");
    }

    Ok(())
}
