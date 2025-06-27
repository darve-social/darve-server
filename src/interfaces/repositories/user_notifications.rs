use async_trait::async_trait;
use serde_json::Value;

use crate::{
    entities::user_notification::UserNotification,
    middleware::{error::AppError, utils::db_utils::QryOrder},
};

pub struct GetNotificationOptions {
    pub limit: u8,
    pub start: u32,
    pub is_read: Option<bool>,
    pub order_dir: QryOrder,
}

#[async_trait]
pub trait UserNotificationsInterface {
    async fn create(
        &self,
        creator: &str,
        title: &str,
        n_type: &str,
        receivers: &Vec<String>,
        metadata: Option<Value>,
    ) -> Result<UserNotification, AppError>;
    async fn get_by_user(
        &self,
        user_id: &str,
        options: GetNotificationOptions,
    ) -> Result<Vec<UserNotification>, AppError>;
    async fn read(&self, id: &str, user_id: &str) -> Result<(), AppError>;
    async fn read_all(&self, user_id: &str) -> Result<(), AppError>;
    async fn get_count(&self, user_id: &str, is_read: Option<bool>) -> Result<u64, AppError>;
}
