mod config;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use storage_api::{create_router, AppState};
use storage_auth::{FirebaseTokenVerifier, SecurityRulesEngine, SignedUrlGenerator};
use storage_core::{BackupService, LocalFilesystemBackend, StorageStore};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::ServerConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "storage_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ServerConfig::load()?;
    info!("Iniciando Moveme Storage em {}", config.listen_addr());
    info!("Firebase project: {}", config.firebase_project_id);

    fs::create_dir_all(&config.data_dir)?;
    fs::create_dir_all(&config.backup_dir)?;

    let backend = Arc::new(LocalFilesystemBackend::new(
        config.data_dir.join("objects"),
    )?);

    let store = Arc::new(StorageStore::new(&config.database_url, backend).await?);

    let backup = Arc::new(BackupService::new(
        config.data_dir.clone(),
        &config.database_url,
        config.backup_dir.clone(),
        config.backup_retention_count,
        store.pool().clone(),
    )?);

    let firebase = Arc::new(FirebaseTokenVerifier::new(&config.firebase_project_id));
    let signed_url = Arc::new(SignedUrlGenerator::new(config.signed_url_secret.as_bytes()));

    let rules = if let Some(path) = &config.security_rules_path {
        let json = fs::read_to_string(path)?;
        Arc::new(SecurityRulesEngine::from_json(&json)?)
    } else {
        Arc::new(SecurityRulesEngine::default_rules())
    };

    if config.backup_auto_interval_hours > 0 {
        let auto_backup = backup.clone();
        let hours = config.backup_auto_interval_hours;
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(hours * 3600));
            interval.tick().await;
            loop {
                interval.tick().await;
                match auto_backup.create_backup(Some("auto".to_string())).await {
                    Ok(b) => info!(
                        backup_id = %b.id,
                        size_bytes = b.size_bytes,
                        "Backup automático criado"
                    ),
                    Err(e) => tracing::error!("Backup automático falhou: {e}"),
                }
            }
        });
        info!(
            "Backup automático ativo a cada {} hora(s)",
            config.backup_auto_interval_hours
        );
    }

    let state = AppState::new(
        store,
        backup,
        firebase,
        signed_url,
        rules,
        config.base_url.clone(),
        config.max_upload_size,
    );

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(&config.listen_addr()).await?;

    info!(
        "Moveme Storage rodando em http://{}",
        config.listen_addr()
    );
    info!("Backups em: {}", config.backup_dir.display());
    info!("Documentação Swagger: http://{}/swagger-ui", config.listen_addr());

    axum::serve(listener, app).await?;

    Ok(())
}
