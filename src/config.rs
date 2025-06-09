#[derive(Debug)]
pub struct AppConfig {
    pub db_namespace: String,
    pub db_database: String,
    pub db_password: Option<String>,
    pub db_username: Option<String>,
    pub db_url: String,
    pub stripe_secret_key: String,
    pub stripe_wh_secret: String,
    pub stripe_platform_account: String,
    pub jwt_secret: String,
    pub upload_file_size_max_mb: u64,
    pub apple_mobile_client_id: String,
    pub code_ttl: u8,
    pub google_client_id: String,
    pub init_server_password: String,
    pub is_development: bool,
    pub sendgrid_api_key: String,
    pub sendgrid_api_url: String,
    pub no_replay: String,
    pub gcs_bucket: String,
    pub gcs_endpoint: Option<String>,
    pub gcs_credentials: Option<String>,
}
impl AppConfig {
    pub fn from_env() -> Self {
        let db_namespace = std::env::var("DB_NAMESPACE").unwrap_or("namespace".to_string());
        let db_database = std::env::var("DB_DATABASE").unwrap_or("database".to_string());
        let db_password = std::env::var("DB_PASSWORD").ok();
        let db_username = std::env::var("DB_USERNAME").ok();
        let db_url = std::env::var("DB_URL").expect("Missing DB_URL in env");

        let stripe_secret_key =
            std::env::var("STRIPE_SECRET_KEY").expect("Missing STRIPE_SECRET_KEY in env");
        let stripe_wh_secret =
            std::env::var("STRIPE_WEBHOOK_SECRET").expect("Missing STRIPE_WEBHOOK_SECRET in env");
        let stripe_platform_account = std::env::var("STRIPE_PLATFORM_ACCOUNT")
            .expect("Missing STRIPE_PLATFORM_ACCOUNT in env");
        let jwt_secret = std::env::var("JWT_SECRET").expect("Missing JWT_SECRET in env");

        let upload_file_size_max_mb: u64 = std::env::var("UPLOAD_MAX_SIZE_MB")
            .unwrap_or("15".to_string())
            .parse()
            .expect("to be number");

        let apple_mobile_client_id =
            std::env::var("APPLE_MOBILE_CLIENT_ID").expect("Missing APPLE_MOBILE_CLIENT_ID in env");

        let code_ttl = std::env::var("EMAIL_CODE_TIME_TO_LIVE")
            .unwrap_or("5".to_string())
            .parse::<u8>()
            .expect("EMAIL_CODE_TIME_TO_LIVE must be number");

        let google_client_id =
            std::env::var("GOOGLE_CLIENT_ID").expect("Missing GOOGLE_CLIENT_ID in env");
        let gcs_bucket =
            std::env::var("GOOGLE_CLOUD_STORAGE_BUCKET").unwrap_or("darve_storage".to_string());
        let gcs_endpoint = std::env::var("GOOGLE_CLOUD_STORAGE_ENDPOINT").ok();
        let gcs_credentials = std::env::var("GOOGLE_CLOUD_STORAGE_CREDENTIALS").ok();

        let init_server_password =
            std::env::var("START_PASSWORD").expect("password to start request");

        let sendgrid_api_key = std::env::var("SENDGRID_API_KEY").unwrap_or_default(); //.expect("SENDGRID_API_KEY must be set");
        let no_replay = std::env::var("NO_REPLY_EMAIL").unwrap_or_default(); //.expect("NO_REPLY_EMAIL must be set");
        let sendgrid_api_url = std::env::var("SENDGRID_API_URL")
            .unwrap_or("https://api.sendgrid.com/v3/mail/send".to_string());

        let is_development = std::env::var("DEVELOPMENT")
            .expect("set DEVELOPMENT env var")
            .eq("true");

        Self {
            db_namespace,
            db_database,
            db_password,
            db_username,
            db_url,
            stripe_secret_key,
            stripe_wh_secret,
            stripe_platform_account,
            jwt_secret,
            upload_file_size_max_mb,
            apple_mobile_client_id,
            code_ttl,
            google_client_id,
            init_server_password,
            is_development,
            sendgrid_api_key,
            sendgrid_api_url,
            no_replay,
            gcs_bucket,
            gcs_endpoint,
            gcs_credentials,
        }
    }
}
