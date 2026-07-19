use serde::Serialize;
use uuid::Uuid;

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct UuidResult {
    pub uuids: Vec<String>,
    pub version: String,
    pub count: usize,
}

pub fn generate_uuids(count: usize, version: &str) -> Result<UuidResult, DevError> {
    if count == 0 || count > 100 {
        return Err(DevError::InvalidInput {
            message: format!("Count must be between 1 and 100, got {count}"),
        });
    }

    let uuids: Vec<String> = match version.to_lowercase().as_str() {
        "v4" | "4" => (0..count).map(|_| Uuid::new_v4().to_string()).collect(),
        "v7" | "7" => (0..count).map(|_| Uuid::now_v7().to_string()).collect(),
        other => {
            return Err(DevError::UnsupportedAlgo {
                algo: format!("UUID version {other} (supported: v4, v7)"),
            })
        }
    };

    Ok(UuidResult {
        count: uuids.len(),
        version: version.to_owned(),
        uuids,
    })
}
