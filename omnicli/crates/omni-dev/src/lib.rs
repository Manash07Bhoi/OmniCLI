pub mod base64;
pub mod error;
pub mod hash;
pub mod json;
pub mod jwt;
pub mod regex_tool;
pub mod uuid_gen;

#[cfg(test)]
mod tests;

pub use base64::{process_base64, Base64Result};
pub use error::DevError;
pub use hash::{compute_hash, HashResult};
pub use json::{process_json, JsonResult};
pub use jwt::{decode_jwt, JwtResult};
pub use regex_tool::{test_regex, RegexResult};
pub use uuid_gen::{generate_uuids, UuidResult};
