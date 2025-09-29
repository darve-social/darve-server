pub fn generate_number_code(count: u8) -> String {
    (0..count)
        .map(|_| rand::random_range(0..10).to_string())
        .collect::<String>()
}
