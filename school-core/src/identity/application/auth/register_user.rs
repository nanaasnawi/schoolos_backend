use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::identity::domain::user::User;
use crate::identity::infrastructure::pg_user_repository::UserRepository;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct RegisterUserCommand {
    pub tenant_id: Uuid,
    pub email: String,
    pub password: String,
    pub full_name: String,
}

pub struct RegisterUserUseCase {
    user_repo: Arc<dyn UserRepository>,
    clock: Arc<dyn Clock>,
}

impl RegisterUserUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { user_repo, clock }
    }

    pub async fn execute(&self, command: RegisterUserCommand) -> Result<User, ApplicationError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(command.password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        let user = User::new(
            command.tenant_id,
            command.email,
            password_hash,
            command.full_name,
            &*self.clock,
        );
        self.user_repo.create(&user).await?;

        Ok(user)
    }
}
