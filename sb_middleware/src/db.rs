use once_cell::sync::Lazy;
use surrealdb::{
    engine::local::{Db as LocalDb, Mem},
    Surreal,
};
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use crate::error::AppResult;

pub type Db = Surreal<Any>;

pub static DB: Lazy<Db> = Lazy::new(Surreal::init);

pub async fn start(db_name: Option<String>) -> AppResult<Surreal<Any>> {
  
    // for in mem db
    // let _db_conn = DB.connect("mem://").await?;
    
    let _db_conn = DB.connect("wss://darvedb-06bbd05cpdpjn2drtcgikpgu5s.aws-euw1.surreal.cloud").await?;

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
    DB.use_ns("darvens")
        .use_db(db_name.unwrap_or("database".to_string()))
        .await?;
    DB.signin(Root {
        username: "test",
        password: "test",
    }).await?;
    Ok(DB.clone())
}
