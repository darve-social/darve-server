use async_trait::async_trait;
use serde_json::Value;

use crate::{entities::user_notification::UserNotification, middleware::error::AppError};
#[async_trait]
pub trait UserNotificationsInterface {
    async fn create(
        &self,
        creator: &str,
        title: &str,
        n_type: &str,
        receivers: &Vec<String>,
        content: Option<String>,
        metadata: Option<Value>,
    ) -> Result<UserNotification, AppError>;
    async fn get_by_user(&self, user_id: &str) -> Result<Vec<UserNotification>, AppError>;
    async fn get_by_id(&self, id: &str, user_id: &str) -> Result<UserNotification, AppError>;
    async fn update(&self, id: &str, is_read: bool) -> Result<(), AppError>;
}
