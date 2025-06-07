pub fn generate_number_code(count: u8) -> String {
    use rand::Rng;
    (0..count)
        .map(|_| {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..10).to_string()
        })
        .collect::<String>()
}
