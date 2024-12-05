pub mod filters {
    use crate::entity::access_rule_entity::AccessRule;
    use crate::entity::authorization_entity::{is_any_ge_in_list, Authorization};
    use rand::Rng;
    use std::fmt::Display;

    const VALUES: [&str; 4] = [
        "This is part of member content, learn more...",
        "Member content, click to get access",
        "Members get access to this content, click to subscribe",
        "Our most valueable topics are visible to members, click to join",
    ];

    // This filter does not have extra arguments
    pub fn keep_alphanumeric<T: std::fmt::Display>(s: T) -> ::askama::Result<String> {
        let s = s.to_string();
        Ok(s.replace(|c: char| !c.is_alphanumeric(), ""))
    }

    pub fn display_some<T: std::fmt::Display>(value: &Option<T>) -> ::askama::Result<String> {
        Ok(match value {
            Some(value) => value.to_string(),
            None => String::new(),
        })
    }

    pub fn if_view_access(
        value: &impl Display,
        replace_with: &str,
        has_view_access: &bool,
    ) -> ::askama::Result<String> {
        match has_view_access {
            true => Ok(format!("{}", value)),
            false => {
                if replace_with.len() > 0 {
                    Ok(format!("{}", replace_with))
                } else {
                    let mut rng = rand::thread_rng();
                    let random_string_index: usize = rng.gen_range(0..VALUES.len());
                    Ok(VALUES[random_string_index].to_string())
                }
            }
        }
    }

    pub fn protected_content_txt(
        value: &impl Display,
        replace_with: String,
        required_access_rule: &Option<AccessRule>,
        viewer_rights: &Vec<Authorization>,
    ) -> ::askama::Result<String> {
        match required_access_rule {
            None => Ok(format!("{}", value)),
            Some(ar) => {
                match is_any_ge_in_list(&ar.authorization_required, &viewer_rights).unwrap_or(false)
                {
                    true => Ok(format!("{}", value)),
                    false => {
                        if replace_with.len() > 0 {
                            Ok(format!("{}", replace_with))
                        } else {
                            let mut rng = rand::thread_rng();
                            let random_string_index: usize = rng.gen_range(0..VALUES.len());
                            Ok(VALUES[random_string_index].to_string())
                        }
                    }
                }
            }
        }
    }
}
