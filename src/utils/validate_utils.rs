use crate::{
    entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
    utils::blocked_words::BLOCKED_WORDS,
};
use chrono::{DateTime, Months, Utc};
use core::fmt;
use regex::Regex;
use reqwest::Url;
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::str::FromStr;
use surrealdb::sql::Thing;
use validator::{ValidateEmail, ValidationError};

pub fn validate_username(u: &String) -> Result<(), ValidationError> {
    let regex = Regex::new(r"^[A-Za-z0-9_]{6,}$").unwrap();

    if !regex.is_match(u) {
        return Err(ValidationError::new("")
            .with_message("Letters, numbers and '_'. Minimum 6 characters".into()));
    }
    let username_lower = u.to_lowercase();
    if BLOCKED_WORDS.contains(&username_lower) {
        return Err(
            ValidationError::new("").with_message("This username contains forbidden words".into())
        );
    }

    Ok(())
}

pub fn validate_phone_number(u: &String) -> Result<(), ValidationError> {
    if Regex::new(r"^\+?[0-9]{7,15}$").unwrap().is_match(u) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_phone_number"))
    }
}

pub fn validate_tags(tags: &[String]) -> Result<(), ValidationError> {
    let rex = Regex::new(r"^[A-Za-z0-9]\w{0,20}$").unwrap();

    for tag in tags {
        let trimmed = tag.trim();

        if trimmed.is_empty() {
            return Err(
                ValidationError::new("invalid_tags").with_message("Tag cannot be empty".into())
            );
        }

        if !rex.is_match(trimmed) {
            return Err(ValidationError::new("invalid_tags")
                .with_message("Tag contains forbidden symbol".into()));
        }
    }
    Ok(())
}

pub fn validate_birth_date(date: &DateTime<Utc>) -> Result<(), ValidationError> {
    let min = Utc::now()
        .checked_sub_months(Months::new(120 * 12))
        .unwrap();
    let max = Utc::now().checked_sub_months(Months::new(10 * 12)).unwrap();
    if *date < min || *date > max {
        return Err(ValidationError::new("invalid_birth_date_range"));
    }
    Ok(())
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

pub fn deserialize_thing_or_string_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct IdExtractor;

    impl<'de> Visitor<'de> for IdExtractor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a Thing object")
        }

        fn visit_str<E>(self, value: &str) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_map<M>(self, map: M) -> Result<String, M::Error>
        where
            M: MapAccess<'de>,
        {
            // Try to deserialize the map as a Thing
            let thing = Thing::deserialize(de::value::MapAccessDeserializer::new(map))?;
            Ok(thing.id.to_raw())
        }
    }

    deserializer.deserialize_any(IdExtractor)
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
            Some(domain) => {
                let mut domain = domain.to_lowercase();
                if let Some(stripped) = domain.strip_prefix("www.") {
                    domain = stripped.to_string();
                }
                domain
            }
            None => return Err(error),
        };

        if !domains.contains(&domain.as_str()) || hash_domains.get(&domain).is_some() {
            return Err(error);
        }

        hash_domains.insert(domain);
    }
    Ok(())
}
