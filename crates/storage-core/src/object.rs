use bytes::Bytes;

use crate::metadata::ObjectMetadata;

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub metadata: ObjectMetadata,
    pub data: Bytes,
}

impl StoredObject {
    pub fn new(metadata: ObjectMetadata, data: Bytes) -> Self {
        Self { metadata, data }
    }
}
