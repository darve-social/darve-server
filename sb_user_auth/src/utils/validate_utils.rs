use std::borrow::Cow;
use std::collections::HashMap;
use validator::{Validate, ValidationError};
pub fn is_some_min_chars(some_str: Option<String>) -> Result<(), ValidationError> {
    if let Some(str) = some_str {
       if str.len()<5 {
           return Err(ValidationError { code: Cow::from("is_some_min_chars"), params: HashMap::new(), message: Some(Cow::from("Value must have min 5 characters." ))});
       }
    }

    Ok(())
}
