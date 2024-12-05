use once_cell::sync::Lazy;
use surrealdb::{
    engine::local::{Db as LocalDb, Mem},
    Surreal,
};

use crate::error::AppResult;

pub type Db = Surreal<LocalDb>;

pub static DB: Lazy<Db> = Lazy::new(Surreal::init);

pub async fn start(db_name: Option<String>) -> AppResult<Surreal<LocalDb>> {
    // DB
    // NOTE: For connection to an existing DB
    // let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 8000));
    // let db = Surreal::new::<Ws>(addr).await?;
    // NOTE: Also possible to start the DB with ::new without a static ::init
    // let db: Db = Surreal::new::<Mem>(()).await?;
    let db_conn = DB.connect::<Mem>(()).await?;

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
    DB.use_ns("namespace")
        .use_db(db_name.unwrap_or("database".to_string()))
        .await?;

    Ok(DB.clone())
}
