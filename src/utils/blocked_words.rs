use std::collections::HashSet;

use once_cell::sync::Lazy;

pub static BLOCKED_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    match File::open("blocked_words.txt") {
        Ok(file) => {
            let reader = BufReader::new(file);

            reader
                .lines()
                .filter_map(|line| line.ok())
                .map(|line| line.to_lowercase())
                .collect()
        }
        Err(_) => HashSet::new(),
    }
});

pub fn init_blocked_words() {
    Lazy::force(&BLOCKED_WORDS);
}
