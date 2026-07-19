use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use serde_json::Value;

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct JwtResult {
    pub valid: bool,
    pub header: Value,
    pub payload: Value,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_expired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn decode_jwt(token: &str) -> Result<JwtResult, DevError> {
    let token = token.trim();
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(DevError::JwtDecode {
            message: format!(
                "Expected 3 dot-separated parts, got {}",
                parts.len()
            ),
        });
    }

    fn decode_part(s: &str) -> Result<Value, DevError> {
        let bytes = URL_SAFE_NO_PAD.decode(s).map_err(|e| DevError::JwtDecode {
            message: format!("Base64url decode failed: {e}"),
        })?;
        serde_json::from_slice(&bytes).map_err(|e| DevError::JwtDecode {
            message: format!("JSON parse failed: {e}"),
        })
    }

    let header = decode_part(parts[0])?;
    let payload = decode_part(parts[1])?;
    let signature = parts[2].to_owned();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let is_expired = payload
        .get("exp")
        .and_then(|v| v.as_u64())
        .map(|exp| now_secs > exp);

    let expires_at = payload
        .get("exp")
        .and_then(|v| v.as_u64())
        .map(|exp| {
            let dt = chrono::DateTime::from_timestamp(exp as i64, 0)
                .unwrap_or_default();
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        });

    let issued_at = payload
        .get("iat")
        .and_then(|v| v.as_u64())
        .map(|iat| {
            let dt = chrono::DateTime::from_timestamp(iat as i64, 0)
                .unwrap_or_default();
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        });

    let subject = payload
        .get("sub")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    Ok(JwtResult {
        valid: true,
        header,
        payload,
        signature,
        is_expired,
        expires_at,
        issued_at,
        subject,
        error: None,
    })
}
