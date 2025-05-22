use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use validator::ValidationError;

pub fn is_some_min_chars(some_str: Option<String>) -> Result<(), ValidationError> {
    if let Some(str) = some_str {
        if str.len() < 5 {
            return Err(ValidationError {
                code: Cow::from("is_some_min_chars"),
                params: HashMap::new(),
                message: Some(Cow::from("Value must have min 5 characters.")),
            });
        }
    }

    Ok(())
}

pub fn validate_username(u: &String) -> Result<(), ValidationError> {
    if Regex::new(r"^[A-Za-z0-9\_]{6,}$").unwrap().is_match(u) {
        Ok(())
    } else {
        let error = ValidationError::new("")
            .with_message("Letters, numbers and '_'. Minimum 6 characters".into());
        Err(error)
    }
}
