use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgres://school_admin:secretpassword@localhost:5433/school_os").await?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password("admin123".as_bytes(), &salt)
        .unwrap()
        .to_string();

    println!("Generated Argon2 password hash: {}", password_hash);

    let rows_affected = sqlx::query("UPDATE users SET password_hash = $1")
        .bind(&password_hash)
        .execute(&pool)
        .await?
        .rows_affected();

    println!("Successfully updated {} users in database to password 'admin123'!", rows_affected);

    Ok(())
}
