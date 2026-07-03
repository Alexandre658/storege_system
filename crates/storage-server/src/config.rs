use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_firebase_project_id")]
    pub firebase_project_id: String,
    #[serde(default = "default_signed_url_secret")]
    pub signed_url_secret: String,
    /// Tamanho máximo de upload em bytes. `None` ou `0` = sem limite.
    #[serde(default)]
    pub max_upload_size: Option<usize>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub security_rules_path: Option<PathBuf>,
    /// Service account (opcional — para operações Admin SDK no futuro)
    #[serde(default)]
    pub firebase_client_email: Option<String>,
    #[serde(default)]
    pub firebase_private_key: Option<String>,
    #[serde(default)]
    pub firebase_api_key: Option<String>,
    #[serde(default)]
    pub firebase_auth_domain: Option<String>,
    #[serde(default)]
    pub firebase_storage_bucket: Option<String>,
    #[serde(default = "default_backup_dir")]
    pub backup_dir: PathBuf,
    #[serde(default = "default_backup_retention")]
    pub backup_retention_count: usize,
    #[serde(default)]
    pub backup_auto_interval_hours: u64,
    #[serde(default = "default_auto_create_buckets")]
    pub auto_create_buckets: bool,
    #[serde(default = "default_bucket_location")]
    pub default_bucket_location: String,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8091
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_database_url() -> String {
    "sqlite://./data/storage.db".to_string()
}

fn default_firebase_project_id() -> String {
    "your-firebase-project-id".to_string()
}

fn default_signed_url_secret() -> String {
    "change-me-signed-url-secret".to_string()
}

fn default_backup_dir() -> PathBuf {
    PathBuf::from("./backups")
}

fn default_backup_retention() -> usize {
    10
}

fn default_base_url() -> String {
    "http://localhost:8091".to_string()
}

fn default_auto_create_buckets() -> bool {
    true
}

fn default_bucket_location() -> String {
    "us-central1".to_string()
}

impl ServerConfig {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::Environment::with_prefix("STORAGE").separator("__"))
            .build()?;

        let mut config: Self = settings.try_deserialize()?;
        config.apply_firebase_env_overrides();
        Ok(config)
    }

    /// Aceita variáveis `FIREBASE_*` diretamente (sem prefixo STORAGE).
    fn apply_firebase_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("FIREBASE_PROJECT_ID") {
            if !v.is_empty() {
                self.firebase_project_id = v;
            }
        }
        if let Ok(v) = std::env::var("FIREBASE_CLIENT_EMAIL") {
            if !v.is_empty() {
                self.firebase_client_email = Some(v);
            }
        }
        if let Ok(v) = std::env::var("FIREBASE_PRIVATE_KEY") {
            if !v.is_empty() {
                self.firebase_private_key = Some(v.replace("\\n", "\n"));
            }
        }
        if let Ok(v) = std::env::var("FIREBASE_API_KEY") {
            if !v.is_empty() {
                self.firebase_api_key = Some(v);
            }
        }
        if let Ok(v) = std::env::var("FIREBASE_AUTH_DOMAIN") {
            if !v.is_empty() {
                self.firebase_auth_domain = Some(v);
            }
        }
        if let Ok(v) = std::env::var("FIREBASE_STORAGE_BUCKET") {
            if !v.is_empty() {
                self.firebase_storage_bucket = Some(v);
            }
        }
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
