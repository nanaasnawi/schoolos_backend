use keyring::Entry;
use tracing::{error, info};

const SERVICE_NAME: &str = "SchoolOS_LocalBridge";
const ACCOUNT_NAME_TOKEN: &str = "agent_auth_token";

pub struct SecureStorage;

impl SecureStorage {
    pub fn save_token(token: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME_TOKEN)?;
        entry.set_password(token)?;
        info!("Agent token securely saved in OS Keychain.");
        Ok(())
    }

    pub fn get_token() -> Result<String, keyring::Error> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME_TOKEN)?;
        match entry.get_password() {
            Ok(token) => Ok(token),
            Err(e) => {
                error!("Failed to retrieve token from secure storage: {:?}", e);
                Err(e)
            }
        }
    }

    pub fn clear_token() -> Result<(), keyring::Error> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME_TOKEN)?;
        entry.delete_password()?;
        info!("Agent token cleared from secure storage.");
        Ok(())
    }
}
