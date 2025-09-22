use crate::config::AppConfig;
use crate::database::client::Database;
use crate::entities::discussion_user::DiscussionUser;
use crate::entities::user_notification::UserNotification;
use crate::interfaces::file_storage::FileStorageInterface;
use crate::interfaces::send_email::SendEmailInterface;
use crate::utils::email_sender::EmailSender;
use crate::utils::file::google_cloud_file_storage::GoogleCloudFileStorage;
use crate::utils::jwt::JWT;
use chrono::Duration;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use surrealdb::sql::Thing;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub enum AppEventType {
    UserNotificationEvent(UserNotification),
    DiscussionPostAdded,
    DiscussionPostReplyAdded,
    DiscussionPostReplyNrIncreased,
    UpdatedUserBalance,
    UpdateDiscussionsUsers(Vec<DiscussionUser>),
    UserStatus(AppEventUsetStatus),
}

#[derive(Debug, Clone, Serialize)]
pub struct AppEventUsetStatus {
    pub is_online: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppEventMetadata {
    pub discussion_id: Option<Thing>,
    pub post_id: Option<Thing>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AppEvent {
    pub user_id: String,
    pub metadata: Option<AppEventMetadata>,
    pub content: Option<String>,
    pub event: AppEventType,
    #[serde(skip_serializing)]
    pub receivers: Vec<String>,
}

pub struct CtxState {
    pub db: Database,
    pub start_password: String,
    pub is_development: bool,
    pub stripe_secret_key: String,
    pub stripe_wh_secret: String,
    pub stripe_platform_account: String,
    pub upload_max_size_mb: u64,
    pub apple_mobile_client_id: String,
    pub google_ios_client_id: String,
    pub google_android_client_id: String,
    pub event_sender: broadcast::Sender<AppEvent>,
    pub verification_code_ttl: Duration,
    pub jwt: JWT,
    pub email_sender: Arc<dyn SendEmailInterface + Send + Sync>,
    pub file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
    pub paypal_webhook_id: String,
    pub paypal_client_id: String,
    pub paypal_client_key: String,
    pub withdraw_fee: f64,
    pub online_users: Arc<DashMap<String, usize>>,
    pub support_email: String,
}

impl Debug for CtxState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("CTX STATE HERE :)")
    }
}

pub async fn create_ctx_state(db: Database, config: &AppConfig) -> Arc<CtxState> {
    let (event_sender, _) = broadcast::channel(100);
    let ctx_state = CtxState {
        db,
        start_password: config.init_server_password.clone(),
        is_development: config.is_development,
        stripe_secret_key: config.stripe_secret_key.clone(),
        stripe_wh_secret: config.stripe_wh_secret.clone(),
        stripe_platform_account: config.stripe_platform_account.clone(),
        upload_max_size_mb: config.upload_file_size_max_mb,
        jwt: JWT::new(config.jwt_secret.clone(), Duration::days(1)),
        apple_mobile_client_id: config.apple_mobile_client_id.clone(),
        google_ios_client_id: config.google_ios_client_id.clone(),
        google_android_client_id: config.google_android_client_id.clone(),
        file_storage: Arc::new(
            GoogleCloudFileStorage::new(
                &config.gcs_bucket,
                config.gcs_credentials.as_deref(),
                config.gcs_endpoint.as_deref(),
            )
            .await,
        ),
        event_sender,
        email_sender: Arc::new(EmailSender::new(
            &config.sendgrid_api_key,
            &config.sendgrid_api_url,
            &config.no_replay,
        )),
        verification_code_ttl: Duration::minutes(config.verification_code_ttl as i64),
        paypal_webhook_id: config.paypal_webhook_id.clone(),
        paypal_client_id: config.paypal_client_id.clone(),
        paypal_client_key: config.paypal_client_key.clone(),
        withdraw_fee: 0.05,
        online_users: Arc::new(DashMap::new()),
        support_email: config.support_email.clone(),
    };
    Arc::new(ctx_state)
}

pub const JWT_KEY: &str = "jwt";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize,
    pub auth: String,
}

#[cfg(test)]
mod tests {
    use crate::middleware::mw_ctx::Claims;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{
        decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
    };

    const SECRET: &[u8] = b"some-secret";
    const SOMEONE: &str = "someone";
    // cspell:disable-next-line
    const TOKEN_EXPIRED: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjEsImF1dGgiOiJzb21lb25lIn0.XXHVHu2IsUPA175aQ-noWbQK4Wu-2prk3qTXjwaWBvE";

    #[test]
    fn jwt_sign_expired() {
        let my_claims = Claims {
            exp: 1,
            auth: SOMEONE.to_string(),
        };
        let token_str = encode(
            &Header::default(),
            &my_claims,
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        assert_eq!(token_str, TOKEN_EXPIRED);
    }

    #[test]
    fn jwt_verify_expired_ignore() {
        let mut validation = Validation::default();
        validation.validate_exp = false;
        let token = decode::<Claims>(
            TOKEN_EXPIRED,
            &DecodingKey::from_secret(SECRET),
            &validation,
        )
        .unwrap();
        assert_eq!(token.claims.auth, SOMEONE);
    }

    #[test]
    fn jwt_verify_expired_fail() {
        let token_result = decode::<Claims>(
            TOKEN_EXPIRED,
            &DecodingKey::from_secret(SECRET),
            &Validation::default(),
        );
        assert!(token_result.is_err());
        let kind = token_result.map_err(|e| e.into_kind()).err();
        assert_eq!(kind, Some(ErrorKind::ExpiredSignature));
    }

    #[test]
    fn jwt_sign_and_verify_with_chrono() {
        let exp = Utc::now() + Duration::minutes(1);
        let my_claims = Claims {
            exp: exp.timestamp() as usize,
            auth: SOMEONE.to_string(),
        };
        // Sign
        let token_str = encode(
            &Header::default(),
            &my_claims,
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        // Verify
        let token_result = decode::<Claims>(
            &token_str,
            &DecodingKey::from_secret(SECRET),
            &Validation::default(),
        )
        .unwrap();
        assert_eq!(token_result.claims.auth, SOMEONE);
    }
}
