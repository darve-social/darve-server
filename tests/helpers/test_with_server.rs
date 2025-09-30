#[warn(unused_imports)]
#[macro_export]
macro_rules! test_with_server {
    ($name:ident, |$server:ident, $ctx_state:ident, $config:ident| $body:block) => {

        #[tokio::test(flavor="multi_thread")]
        #[serial_test::serial]
        async fn $name() {
            use std::sync::Arc;
            use dashmap::DashMap;
            use async_trait::async_trait;
            use darve_server::interfaces::send_email::SendEmailInterface;
            use darve_server::{
                middleware::mw_ctx::CtxState,
                utils::{file::local_file_storage::LocalFileStorage, jwt::JWT},
            };
            use tokio::sync::broadcast;
            use axum_test::{TestServer, TestServerConfig};
            use darve_server::database::client::{Database, DbConfig};
            use darve_server::config::AppConfig;
            use darve_server::routes::user_auth::webauthn::webauthn_routes::create_webauth_config;
            use futures::FutureExt;
            use std::panic::{ resume_unwind};


            struct MockEmailSender;

            #[async_trait]
            impl SendEmailInterface for MockEmailSender {
                async fn send(&self, _emails: Vec<String>, _body: &str, _subject: &str) -> Result<(), String> {
                    Ok(())
                }
            }

            fn create_ctx_state(db: Database, config: &AppConfig) -> Arc<CtxState> {
                let (event_sender, _) = broadcast::channel(100);
                let ctx_state = CtxState {
                    db,
                    start_password: config.init_server_password.clone(),
                    is_development: config.is_development,
                    stripe_secret_key: config.stripe_secret_key.clone(),
                    stripe_wh_secret: config.stripe_wh_secret.clone(),
                    stripe_platform_account: config.stripe_platform_account.clone(),
                    upload_max_size_mb: config.upload_file_size_max_mb,
                    jwt: JWT::new(config.jwt_secret.clone(), chrono::Duration::days(1)),
                    apple_mobile_client_id: config.apple_mobile_client_id.clone(),
                    google_ios_client_id: config.google_ios_client_id.clone(),
                    google_android_client_id: config.google_android_client_id.clone(),
                    file_storage: Arc::new(LocalFileStorage::new("target/tests_media".to_string(), "".to_string())),
                    email_sender: Arc::new(MockEmailSender {}),
                    verification_code_ttl: chrono::Duration::minutes(config.verification_code_ttl as i64),
                    paypal_webhook_id: config.paypal_webhook_id.clone(),
                    paypal_client_id: config.paypal_client_id.clone(),
                    paypal_client_key: config.paypal_client_key.clone(),
                    event_sender,
                    withdraw_fee: 0.05,
                    online_users: Arc::new(DashMap::new()),
                    support_email: config.support_email.clone()
                };
                Arc::new(ctx_state)
            }

            let $config = AppConfig {
                db_namespace: "test".to_string(),
                db_database: "test".to_string(),
                db_password: None,
                db_username: None,
                db_url: "mem://".to_string(),
                stripe_secret_key: "".to_string(),
                stripe_wh_secret: "".to_string(),
                stripe_platform_account: "".to_string(),
                jwt_secret: "secret".to_string(),
                upload_file_size_max_mb: 100,
                apple_mobile_client_id: "".to_string(),
                verification_code_ttl: 3,
                google_ios_client_id: "".to_string(),
                google_android_client_id: "".to_string(),
                init_server_password: "".to_string(),
                is_development: true,
                sendgrid_api_key: "".to_string(),
                sendgrid_api_url: "".to_string(),
                no_replay: "".to_string(),
                gcs_bucket: "".to_string(),
                gcs_endpoint: None,
                gcs_credentials: None,
                sentry_project_link: None,
                paypal_webhook_id: "".to_string(),
                paypal_client_id: "".to_string(),
                paypal_client_key: "".to_string(),
                support_email: "".to_string(),
                rate_limit_rsp: 1000,
                rate_limit_burst: 1000
            };

            let $ctx_state = {
                let db = Database::connect(DbConfig {
                    url: &$config.db_url,
                    database: &$config.db_database,
                    namespace: &$config.db_namespace,
                    password: $config.db_password.as_deref(),
                    username: $config.db_username.as_deref(),
                })
                .await;

                db.run_migrations().await.unwrap();
                darve_server::init::run_migrations(&db).await.unwrap();
                create_ctx_state(db, &$config)
            };

            let wa_config = create_webauth_config();
            let routes_all = darve_server::init::main_router(&$ctx_state.clone(), wa_config, &$config);

            let $server = TestServer::new_with_config(
                routes_all,
                TestServerConfig {
                    transport: None,
                    save_cookies: true,
                    expect_success_by_default: false,
                    restrict_requests_with_http_schema: false,
                    default_content_type: None,
                    default_scheme: None,
                },
            )
            .expect("Failed to create test server");

            let test_result = std::panic::AssertUnwindSafe(async {
                (|| async $body)().await;
            })
            .catch_unwind()
            .await;

            $ctx_state.clone().db.client
                .query(format!("REMOVE DATABASE {};",$config.db_database))
                .await
                .expect("failed to remove database");

            if let Err(panic) = test_result {
                resume_unwind(panic);
            }
        }
    };
}
