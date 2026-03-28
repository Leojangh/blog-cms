use argon2::{password_hash::SaltString, Argon2, PasswordHasher};

fn main() {
    let password = std::env::args().nth(1).expect("Usage: hash_password <password>");
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt).unwrap();
    println!("{hash}");
}
