use api_server::bootstrap;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_server=debug,school_core=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let bootstrap = bootstrap::Bootstrap::new();
    let app = bootstrap.build().await?;

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Server running on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;
    #[test]
    fn export_openapi() {
        let openapi = ApiDoc::openapi();
        let json = openapi.to_json().unwrap();
        let _ = std::fs::write("../../frontend/openapi.json", &json);
        let _ = std::fs::create_dir_all("../../docs/api-contract/contracts");
        let _ = std::fs::write("../../docs/api-contract/openapi.json", &json);
        let _ = std::fs::write("../../docs/api-contract/contracts/openapi.json", &json);
    }
}
