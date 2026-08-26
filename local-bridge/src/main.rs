pub mod domain;
pub mod store;
pub mod auth;
pub mod dapodik_acl;
pub mod sync;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use sync::engine::SyncEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("Starting School OS Local Bridge Agent...");

    // TODO: Initialize OS Secure Storage
    // TODO: Initialize Local SQLite Database
    // TODO: Initialize Dapodik ACL
    
    // Start Sync Engine (PULL & PUSH loops)
    SyncEngine::start();

    // Keep the daemon running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Local Bridge Agent.");

    Ok(())
}
