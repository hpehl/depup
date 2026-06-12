use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum MvnupErrorCode {
    PomNotFound,
    PomParseFailed,
    RegistryLookupFailed,
    NoVersionsFound,
    HttpRequestFailed,
    ClapParseError,
    Internal,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
#[must_use]
pub struct MvnupError {
    pub code: MvnupErrorCode,
    pub message: String,
}

#[allow(dead_code)]
impl MvnupError {
    pub fn pom_not_found(path: &str) -> Self {
        Self {
            code: MvnupErrorCode::PomNotFound,
            message: format!("No pom.xml found in {path}"),
        }
    }

    pub fn pom_parse_failed(path: &str, detail: &str) -> Self {
        Self {
            code: MvnupErrorCode::PomParseFailed,
            message: format!("Failed to parse {path}: {detail}"),
        }
    }

    pub fn registry_lookup_failed(artifact: &str, detail: &str) -> Self {
        Self {
            code: MvnupErrorCode::RegistryLookupFailed,
            message: format!("Registry lookup failed for {artifact}: {detail}"),
        }
    }

    pub fn no_versions_found(artifact: &str) -> Self {
        Self {
            code: MvnupErrorCode::NoVersionsFound,
            message: format!("No versions found for {artifact}"),
        }
    }

    pub fn http_request_failed(url: &str, detail: &str) -> Self {
        Self {
            code: MvnupErrorCode::HttpRequestFailed,
            message: format!("HTTP request failed for {url}: {detail}"),
        }
    }

    pub fn clap_parse_error(detail: &str) -> Self {
        Self {
            code: MvnupErrorCode::ClapParseError,
            message: detail.to_string(),
        }
    }

    pub fn error_code(err: &anyhow::Error) -> MvnupErrorCode {
        err.downcast_ref::<MvnupError>()
            .map(|e| e.code)
            .unwrap_or(MvnupErrorCode::Internal)
    }
}

#[derive(Serialize)]
pub struct JsonErrorEnvelope {
    pub error: JsonErrorBody,
}

#[derive(Serialize)]
pub struct JsonErrorBody {
    pub code: MvnupErrorCode,
    pub message: String,
}

impl JsonErrorEnvelope {
    pub fn from_mvnup_error(err: &MvnupError) -> Self {
        Self {
            error: JsonErrorBody {
                code: err.code,
                message: err.message.clone(),
            },
        }
    }

    pub fn from_anyhow(err: &anyhow::Error) -> Self {
        match err.downcast_ref::<MvnupError>() {
            Some(e) => Self::from_mvnup_error(e),
            None => Self {
                error: JsonErrorBody {
                    code: MvnupErrorCode::Internal,
                    message: err.to_string(),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_serializes_as_screaming_snake_case() {
        let json = serde_json::to_string(&MvnupErrorCode::PomNotFound).unwrap();
        assert_eq!(json, "\"POM_NOT_FOUND\"");
    }

    #[test]
    fn error_code_http_request_failed() {
        let json = serde_json::to_string(&MvnupErrorCode::HttpRequestFailed).unwrap();
        assert_eq!(json, "\"HTTP_REQUEST_FAILED\"");
    }

    #[test]
    fn mvnup_error_display_uses_message() {
        let err = MvnupError::pom_not_found("/some/path");
        assert_eq!(err.to_string(), "No pom.xml found in /some/path");
    }

    #[test]
    fn mvnup_error_parameterized_message() {
        let err = MvnupError::http_request_failed("https://repo.example.com", "timeout");
        assert_eq!(
            err.to_string(),
            "HTTP request failed for https://repo.example.com: timeout"
        );
    }

    #[test]
    fn json_error_envelope_from_mvnup_error() {
        let err = MvnupError::no_versions_found("org.example:my-lib");
        let envelope = JsonErrorEnvelope::from_mvnup_error(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "NO_VERSIONS_FOUND");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("org.example:my-lib"));
    }

    #[test]
    fn json_error_envelope_from_anyhow_with_mvnup_error() {
        let err: anyhow::Error = MvnupError::pom_parse_failed("pom.xml", "invalid XML").into();
        let envelope = JsonErrorEnvelope::from_anyhow(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "POM_PARSE_FAILED");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid XML"));
    }

    #[test]
    fn json_error_envelope_from_anyhow_without_mvnup_error() {
        let err = anyhow::anyhow!("something unexpected");
        let envelope = JsonErrorEnvelope::from_anyhow(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "INTERNAL");
        assert_eq!(json["error"]["message"], "something unexpected");
    }

    #[test]
    fn error_code_extracts_from_anyhow() {
        let err: anyhow::Error =
            MvnupError::registry_lookup_failed("org.example:lib", "connection refused").into();
        assert_eq!(
            MvnupError::error_code(&err),
            MvnupErrorCode::RegistryLookupFailed
        );
    }

    #[test]
    fn error_code_falls_back_to_internal() {
        let err = anyhow::anyhow!("plain error");
        assert_eq!(MvnupError::error_code(&err), MvnupErrorCode::Internal);
    }
}
