use std::sync::Arc;

use storage_auth::{SecurityRulesEngine, SignedUrlGenerator, FirebaseTokenVerifier};
use storage_core::{BackupService, StorageStore};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<StorageStore>,
    pub backup: Arc<BackupService>,
    pub firebase: Arc<FirebaseTokenVerifier>,
    pub signed_url: Arc<SignedUrlGenerator>,
    pub rules: Arc<SecurityRulesEngine>,
    pub base_url: String,
    pub max_upload_size: usize,
}

impl AppState {
    pub fn new(
        store: Arc<StorageStore>,
        backup: Arc<BackupService>,
        firebase: Arc<FirebaseTokenVerifier>,
        signed_url: Arc<SignedUrlGenerator>,
        rules: Arc<SecurityRulesEngine>,
        base_url: String,
        max_upload_size: usize,
    ) -> Self {
        Self {
            store,
            backup,
            firebase,
            signed_url,
            rules,
            base_url,
            max_upload_size,
        }
    }
}
