use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum DepupErrorCode {
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

    #[cfg(test)]
    pub fn pom_parse_failed(path: &str, detail: &str) -> Self {
        Self {
            code: DepupErrorCode::PomParseFailed,
            message: format!("Failed to parse {path}: {detail}"),
        }
    }

    #[cfg(test)]
    pub fn registry_lookup_failed(artifact: &str, detail: &str) -> Self {
        Self {
            code: DepupErrorCode::RegistryLookupFailed,
            message: format!("Registry lookup failed for {artifact}: {detail}"),
        }
    }

    #[cfg(test)]
    pub fn no_versions_found(artifact: &str) -> Self {
        Self {
            code: DepupErrorCode::NoVersionsFound,
            message: format!("No versions found for {artifact}"),
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

#[derive(Serialize)]
pub struct JsonErrorEnvelope {
    pub error: JsonErrorBody,
}

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
        let err = DepupError::no_versions_found("org.example:my-lib");
        let envelope = JsonErrorEnvelope::from_depup_error(&err);
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
        assert_eq!(json["error"]["code"], "NO_VERSIONS_FOUND");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("org.example:my-lib")
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
        let err: anyhow::Error =
            DepupError::registry_lookup_failed("org.example:lib", "connection refused").into();
        assert_eq!(
            DepupError::error_code(&err),
            DepupErrorCode::RegistryLookupFailed
        );
    }

    #[test]
    fn error_code_falls_back_to_internal() {
        let err = anyhow::anyhow!("plain error");
        assert_eq!(DepupError::error_code(&err), DepupErrorCode::Internal);
    }
}
