use once_cell::sync::Lazy;
use surrealdb::{
    Surreal
};
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use crate::error::AppResult;

pub type Db = Surreal<Any>;

pub static DB: Lazy<Db> = Lazy::new(Surreal::init);

#[derive(Debug)]
pub struct DBConfig {
    pub namespace: String,
    pub database: String,
    pub password: Option<String>,
    pub username: Option<String>,
    pub url: String,
}

impl DBConfig {
    pub fn from_env() -> Self {
        let namespace = std::env::var("DB_NAMESPACE").unwrap_or("namespace".to_string());
        let database = std::env::var("DB_DATABASE").unwrap_or("database".to_string());
        let password = std::env::var("DB_PASSWORD").ok();
        let username = std::env::var("DB_USERNAME").ok();
        let url = std::env::var("DB_URL").unwrap_or_else(|_| "mem://".to_string());

        Self {
            namespace,
            database,
            password,
            username,
            url,
        }
    }
    
}

pub async fn start(config: DBConfig) -> AppResult<Db> {
    let _conn = DB.connect(config.url).await?;

    match (config.password.as_ref(), config.username.as_ref()) {
        (Some(password), Some(username)) => {
            DB.signin(Root {
                username,
                password,
            }).await?;
        }
        _ => {}
    }

    DB.use_ns(config.namespace)
        .use_db(config.database)
        .await?;

  
    // for in mem db
    // let _db_conn = DB.connect("mem://").await?;
    
    // let _db_conn = DB.connect("wss://darvedb-06bbd05cpdpjn2drtcgikpgu5s.aws-euw1.surreal.cloud").await?;

    /*if let Some(SurrealErr::Api(err)) = db_conn.as_ref().err() {
        match err {
            SDB_ApiError::AlreadyConnected => println!("surrealdb ERR = {:?}", err.clone()),
            _ => return Err(db_conn.err().unwrap().into())
        }
    }*/

    println!("->> DB connected in memory");
    let version = DB.version().await?;
    println!("->> DB version: {version}");
    // Select a specific namespace / database
    // DB.use_ns("darvens")
    //     .use_db(db_name.unwrap_or("database".to_string()))
    //     .await?;
    // DB.signin(Root {
    //     username: "test",
    //     password: "test",
    // }).await?;
    Ok(DB.clone())
}
