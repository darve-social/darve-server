#[warn(unused_imports)]
#[macro_export]
macro_rules! test_with_server {
    ($name:ident, |$server:ident, $ctx_state:ident, $config:ident| $body:block) => {

        #[tokio::test(flavor="multi_thread")]
        #[serial_test::serial]
        async fn $name() {
            use axum_test::{TestServer, TestServerConfig};
            use darve_server::database::client::{Database, DbConfig};
            use darve_server::config::AppConfig;
            use darve_server::middleware::mw_ctx::create_ctx_state;
            use darve_server::routes::user_auth::webauthn::webauthn_routes::create_webauth_config;
            use futures::FutureExt;
            use std::panic::{ resume_unwind};

            let $config = AppConfig::from_env();

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

                create_ctx_state(db, &$config).await
            };

            let wa_config = create_webauth_config();
            let routes_all = darve_server::init::main_router(&$ctx_state.clone(), wa_config).await;

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
