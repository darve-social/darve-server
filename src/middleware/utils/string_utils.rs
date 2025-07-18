use crate::middleware::error::{AppError, AppResult};
use surrealdb::sql::Thing;

pub fn get_string_thing(value: String) -> AppResult<Thing> {
    Thing::try_from(value).map_err(|_| AppError::Generic {
        description: "error into Thing".to_string(),
    })
}

pub fn get_str_thing(value: &str) -> AppResult<Thing> {
    Thing::try_from(value).map_err(|_| AppError::Generic {
        description: "error into Thing".to_string(),
    })
}

pub const LEN_OR_NONE: fn(v: String) -> Option<String> =
    |v| if v.len() > 0 { Some(v) } else { None };
