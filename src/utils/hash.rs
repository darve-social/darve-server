use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

pub fn hash_password(pwd: &str) -> Result<(String, String), String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let result = argon2.hash_password(pwd.as_bytes(), &salt);
    match result {
        Ok(hash) => Ok((hash.algorithm.to_string(), hash.to_string())),
        Err(err) => Err(err.to_string()),
    }
}

pub fn verify_password(hash: &str, pwd: &str) -> bool {
    let parsed_hash = PasswordHash::new(hash);

    if parsed_hash.is_err() {
        return false;
    }
    let argon2 = Argon2::default();
    let res = argon2.verify_password(pwd.as_bytes(), &parsed_hash.unwrap());
    match res {
        Ok(_) => true,
        Err(_) => false,
    }
}
