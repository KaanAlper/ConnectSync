use yup_oauth2::storage::{TokenStorage, TokenInfo};
use yup_oauth2::storage::TokenStorageError;
use async_trait::async_trait;

struct MyStorage;
#[async_trait]
impl TokenStorage for MyStorage {
    async fn set(&self, scopes: &[&str], token: TokenInfo) -> Result<(), TokenStorageError> {
        Ok(())
    }
    async fn get(&self, scopes: &[&str]) -> Option<TokenInfo> {
        None
    }
}
