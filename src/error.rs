//! Structured error types for human-readable and machine-consumable error output.
//!
//! Every `DepupError` carries a stable `DepupErrorCode` that serializes to
//! `SCREAMING_SNAKE_CASE` for the JSON error envelope, making errors parseable
//! by CI scripts and other tools.
//!
//! ## Error handling strategy
//!
//! - **`DepupError`** — Use for errors that reach the CLI surface and need a
//!   stable, machine-readable code (POM not found, HTTP failure, parse error).
//!   These are wrapped in `JsonErrorEnvelope` when `--json` is active.
//!
//! - **`anyhow::Result`** — Use for internal plumbing where a stable error code
//!   is not needed. The top-level handler in `main.rs` downcasts to `DepupError`
//!   when possible, falling back to `Internal` for plain `anyhow` errors.

use serde::Serialize;

/// Stable, machine-readable error codes. Serialized as `SCREAMING_SNAKE_CASE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DepupErrorCode {
    PomNotFound,
    PomParseFailed,
    HttpRequestFailed,
    ClapParseError,
    Internal,
}

/// Application-specific error with a stable code for machine consumption.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
#[must_use]
pub struct DepupError {
    pub code: DepupErrorCode,
    pub message: String,
}

impl DepupError {
    pub fn pom_not_found(path: &str) -> Self {
        Self {
            code: DepupErrorCode::PomNotFound,
            message: format!("No pom.xml found in {path}"),
        }
    }

    pub fn pom_parse_failed(path: &str, detail: &str) -> Self {
        Self {
            code: DepupErrorCode::PomParseFailed,
            message: format!("Failed to parse {path}: {detail}"),
        }
    }

    pub fn http_request_failed(url: &str, detail: &str) -> Self {
        Self {
            code: DepupErrorCode::HttpRequestFailed,
            message: format!("HTTP request failed for {url}: {detail}"),
        }
    }

    pub fn clap_parse_error(detail: &str) -> Self {
        Self {
            code: DepupErrorCode::ClapParseError,
            message: detail.to_string(),
        }
    }

    #[cfg(test)]
    pub fn error_code(err: &anyhow::Error) -> DepupErrorCode {
        err.downcast_ref::<Self>()
            .map_or(DepupErrorCode::Internal, |e| e.code)
    }
}

/// JSON envelope wrapping an error for `--json` mode output.
#[derive(Serialize)]
pub struct JsonErrorEnvelope {
    pub error: JsonErrorBody,
}

/// Inner body of the JSON error envelope.
#[derive(Serialize)]
pub struct JsonErrorBody {
    pub code: DepupErrorCode,
    pub message: String,
}

impl JsonErrorEnvelope {
    pub fn from_depup_error(err: &DepupError) -> Self {
        Self {
            error: JsonErrorBody {
                code: err.code,
                message: err.message.clone(),
            },
        }
    }

    pub fn from_anyhow(err: &anyhow::Error) -> Self {
        err.downcast_ref::<DepupError>().map_or_else(
            || Self {
                error: JsonErrorBody {
                    code: DepupErrorCode::Internal,
                    message: err.to_string(),
                },
            },
            Self::from_depup_error,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_serializes_as_screaming_snake_case() {
        let json = serde_json::to_string(&DepupErrorCode::PomNotFound).unwrap();
        assert_eq!(json, "\"POM_NOT_FOUND\"");
    }

    #[test]
    fn error_code_http_request_failed() {
        let json = serde_json::to_string(&DepupErrorCode::HttpRequestFailed).unwrap();
        assert_eq!(json, "\"HTTP_REQUEST_FAILED\"");
    }

    #[test]
    fn depup_error_display_uses_message() {
        let err = DepupError::pom_not_found("/some/path");
        assert_eq!(err.to_string(), "No pom.xml found in /some/path");
    }

    #[test]
    fn depup_error_parameterized_message() {
        let err = DepupError::http_request_failed("https://repo.example.com", "timeout");
        assert_eq!(
            err.to_string(),
            "HTTP request failed for https://repo.example.com: timeout"
        );
    }

    #[test]
    fn json_error_envelope_from_depup_error() {
        let err = DepupError::pom_not_found("/some/path");
        let envelope = JsonErrorEnvelope::from_depup_error(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "POM_NOT_FOUND");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("/some/path")
        );
    }

    #[test]
    fn json_error_envelope_from_anyhow_with_depup_error() {
        let err: anyhow::Error = DepupError::pom_parse_failed("pom.xml", "invalid XML").into();
        let envelope = JsonErrorEnvelope::from_anyhow(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "POM_PARSE_FAILED");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("invalid XML")
        );
    }

    #[test]
    fn json_error_envelope_from_anyhow_without_depup_error() {
        let err = anyhow::anyhow!("something unexpected");
        let envelope = JsonErrorEnvelope::from_anyhow(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "INTERNAL");
        assert_eq!(json["error"]["message"], "something unexpected");
    }

    #[test]
    fn error_code_extracts_from_anyhow() {
        let err: anyhow::Error = DepupError::pom_parse_failed("pom.xml", "invalid XML").into();
        assert_eq!(DepupError::error_code(&err), DepupErrorCode::PomParseFailed);
    }

    #[test]
    fn error_code_falls_back_to_internal() {
        let err = anyhow::anyhow!("plain error");
        assert_eq!(DepupError::error_code(&err), DepupErrorCode::Internal);
    }
}
