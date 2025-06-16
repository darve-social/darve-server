use regex::Regex;
use serde::{de, Deserialize, Deserializer};
use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use surrealdb::sql::Thing;
use validator::{ValidateEmail, ValidationError};

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

pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}

pub fn deserialize_thing_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let thing = Thing::deserialize(deserializer)?;
    Ok(thing.to_raw())
}

pub fn deserialize_option_string_id<'de, D>(deserializer: D) -> Result<Option<Thing>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.trim().is_empty() => Ok(Thing::from_str(&s)
            .map(Some)
            .map_err(|_| de::Error::custom("Invalid id"))?),
        _ => Ok(None),
    }
}

pub fn validate_email_or_username(value: &str) -> Result<(), ValidationError> {
    if value.validate_email() {
        return Ok(());
    }
    if validate_username(&value.to_string()).is_ok() {
        return Ok(());
    }
    Err(ValidationError::new("email_or_username"))
}
