use yup_oauth2::storage::{TokenStorage, TokenInfo};
use yup_oauth2::storage::TokenStorageError;
use async_trait::async_trait;

#[allow(dead_code)]
struct MyStorage;
#[async_trait]
impl TokenStorage for MyStorage {
    async fn set(&self, _scopes: &[&str], _token: TokenInfo) -> Result<(), TokenStorageError> {
        Ok(())
    }
    async fn get(&self, _scopes: &[&str]) -> Option<TokenInfo> {
        None
    }
}
