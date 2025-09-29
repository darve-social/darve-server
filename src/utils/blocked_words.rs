use std::collections::HashSet;

use once_cell::sync::Lazy;

pub static BLOCKED_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    let data = include_str!("../../blocked_words.txt");
    data.lines().map(|line| line.to_lowercase()).collect()
});
