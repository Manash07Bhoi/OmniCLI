use regex::Regex;
use serde::Serialize;

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct RegexMatch {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub groups: Vec<Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct RegexResult {
    pub valid: bool,
    pub pattern: String,
    pub flags: String,
    pub match_count: usize,
    pub matches: Vec<RegexMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn test_regex(pattern: &str, text: &str, flags: &str) -> Result<RegexResult, DevError> {
    // Build regex with optional flags
    let pattern_with_flags = if flags.is_empty() {
        pattern.to_owned()
    } else {
        let mut prefix = String::from("(?");
        for ch in flags.chars() {
            match ch {
                'i' | 'm' | 's' | 'x' => prefix.push(ch),
                _ => {}
            }
        }
        prefix.push(')');
        format!("{prefix}{pattern}")
    };

    let re = match Regex::new(&pattern_with_flags) {
        Ok(r) => r,
        Err(e) => {
            return Ok(RegexResult {
                valid: false,
                pattern: pattern.to_owned(),
                flags: flags.to_owned(),
                match_count: 0,
                matches: vec![],
                error: Some(e.to_string()),
            })
        }
    };

    let matches: Vec<RegexMatch> = re
        .captures_iter(text)
        .map(|cap| {
            let m = cap.get(0).unwrap();
            let groups = (1..cap.len())
                .map(|i| cap.get(i).map(|g| g.as_str().to_owned()))
                .collect();
            RegexMatch {
                text: m.as_str().to_owned(),
                start: m.start(),
                end: m.end(),
                groups,
            }
        })
        .collect();

    let match_count = matches.len();
    Ok(RegexResult {
        valid: true,
        pattern: pattern.to_owned(),
        flags: flags.to_owned(),
        match_count,
        matches,
        error: None,
    })
}
