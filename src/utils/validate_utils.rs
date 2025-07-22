use crate::entities::community::discussion_entity::USER_TABLE_NAME;
use regex::Regex;
use reqwest::Url;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use surrealdb::sql::Thing;
use validator::{ValidateEmail, ValidationError};

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
    Ok(thing.id.to_raw())
}

pub fn serialize_string_id<S>(x: &String, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match x.is_empty() {
        true => s.serialize_none(),
        false => s.serialize_str(x.as_str()),
    }
}

pub fn serialize_to_user_thing<S>(x: &String, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match x.is_empty() {
        true => s.serialize_none(),
        false => Thing::from((USER_TABLE_NAME, x.as_str())).serialize(s),
    }
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

pub fn validate_social_links(social_links: &[String]) -> Result<(), validator::ValidationError> {
    let mut hash_domains = std::collections::HashSet::new();

    let domains = ["x.com", "instagram.com", "youtube.com", "facebook.com"];

    let mut error = validator::ValidationError::new("invalid_social_link");
    error.message =
        Some("Social link must be from Twitter, Instagram, YouTube, or Facebook".into());

    for link in social_links {
        let parsed_url = match Url::parse(link) {
            Ok(url) => url,
            Err(_) => return Err(error),
        };

        if parsed_url.scheme() != "https" {
            return Err(error);
        }
        let domain = match parsed_url.domain() {
            Some(domain) => domain.to_lowercase(),
            None => return Err(error),
        };

        if !domains.contains(&domain.as_str()) || hash_domains.get(&domain).is_some() {
            return Err(error);
        }

        hash_domains.insert(domain);
    }
    Ok(())
}
