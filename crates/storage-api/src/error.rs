use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use storage_core::StorageError;

use crate::dto::{ErrorDetail, ErrorResponse};

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: msg.into(),
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::NotFound(msg) | StorageError::BucketNotFound(msg) | StorageError::UploadSessionNotFound(msg) => {
                ApiError::not_found(msg)
            }
            StorageError::Forbidden(msg) => ApiError::forbidden(msg),
            StorageError::BadRequest(msg) | StorageError::UploadSessionExpired(msg) => {
                ApiError::bad_request(msg)
            }
            StorageError::AlreadyExists(msg) => ApiError {
                status: StatusCode::CONFLICT,
                message: msg,
            },
            _ => ApiError::internal(err.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_str = match self.status {
            StatusCode::BAD_REQUEST => "INVALID_ARGUMENT",
            StatusCode::UNAUTHORIZED => "UNAUTHENTICATED",
            StatusCode::FORBIDDEN => "PERMISSION_DENIED",
            StatusCode::NOT_FOUND => "NOT_FOUND",
            StatusCode::CONFLICT => "ALREADY_EXISTS",
            _ => "INTERNAL",
        };

        let body = ErrorResponse {
            error: ErrorDetail {
                code: self.status.as_u16(),
                message: self.message,
                status: status_str.to_string(),
            },
        };

        (self.status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
