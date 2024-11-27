use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, get_entity_view, record_exists, with_not_found_err, IdentIdName, ViewFieldSelector};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::sql::Thing;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wallet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub user: Thing,
    pub transaction_head: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive( Display, Clone,Serialize, Deserialize, Debug)]
pub enum CurrencySymbol {
    USD
}

#[derive(Deserialize, Debug)]
pub struct WalletBalanceView {
    pub id: Thing,
    pub user_id: Thing,
    pub balance: i64,
    pub currency_symbol: CurrencySymbol,
}

impl ViewFieldSelector for WalletBalanceView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        format!("id, user as user_id, {TRANSACTION_HEAD_F}.*.balance as balance, {TRANSACTION_HEAD_F}.*.currency as currency_symbol")
    }
}

pub struct WalletDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
    pub is_development: bool,
}

pub const TABLE_NAME: &str = "wallet";
const USER_TABLE: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TRANSACTION_TABLE: &str = crate::entity::currency_transaction_entitiy::TABLE_NAME;

pub const TRANSACTION_HEAD_F: &str = "transaction_head";

impl<'a> WalletDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE}> VALUE $before OR $value;
    DEFINE FIELD {TRANSACTION_HEAD_F} ON TABLE {TABLE_NAME} TYPE record<{TRANSACTION_TABLE}>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate wallet");

        Ok(())
    }

    // creates wallet
    pub async fn get_balance(&self, user_id: Thing) -> CtxResult<WalletBalanceView> {
        let user_wallet_id = Self::get_wallet_id(&user_id);
        if record_exists(self.db, user_wallet_id.clone()).await.is_ok() {
            self.get_view::<WalletBalanceView>(IdentIdName::Id(user_wallet_id)).await
        } else {
            self.init_wallet(user_id).await
        }
    }

    async fn init_wallet(&self, user_id: Thing) -> CtxResult<WalletBalanceView> {
        let user_wallet_id = Self::get_wallet_id(&user_id);
        if record_exists(self.db, user_wallet_id.clone()).await.is_ok() {
            return Err(self.ctx.to_ctx_error(AppError::Generic { description: "Wallet already exists".to_string() }));
        }

        let currency_symbol = CurrencySymbol::USD;
        let balance = if self.is_development {
            Some(100)
        } else { None };
        let init_tx = CurrencyTransactionDbService { db: self.db, ctx: self.ctx }.create_init_record(user_wallet_id.clone(), currency_symbol.clone(), balance).await?;
        let wallet = self.db
            .create(TABLE_NAME)
            .content(Wallet {
                id: Some(user_wallet_id),
                user: user_id.clone(),
                transaction_head: init_tx.id.unwrap(),
                r_created: None,
                r_updated: None,
            })
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Wallet>| v.unwrap())?;
        Ok(WalletBalanceView {
            id: wallet.id.unwrap(),
            user_id,
            balance: init_tx.balance,
            currency_symbol,
        })
    }

    fn get_wallet_id(user_id: &Thing) -> Thing {
        Thing::from((TABLE_NAME, user_id.id.to_raw().as_str()))
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, ident_id_name: IdentIdName) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<Wallet> {
        let opt = get_entity::<Wallet>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::fmt::Formatter;
    use serde::{Deserialize, Serialize};
    use strum::{Display, EnumString};
    use crate::entity::currency_transaction_entitiy::CurrencyTransactionDbService;
    use crate::entity::wallet_entitiy::{CurrencySymbol, WalletDbService};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::db;
    use sb_middleware::error::AppResult;
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::authentication_entity::AuthType;
    use sb_user_auth::entity::local_user_entity::{LocalUser, LocalUserDbService};
    use surrealdb::engine::local::Db;
    use surrealdb::{Surreal, Uuid};
    use surrealdb::sql::Thing;
    use tokio::io::AsyncWriteExt;
    use tokio_stream::StreamExt;
    use sb_middleware::utils::db_utils::IdentIdName;

    #[tokio::test]
    async fn query_with_params() {

        let (db, ctx) = init_db_test().await;

        let usr1 = LocalUserDbService { db: &db, ctx: &ctx }.create(LocalUser {
            id: None,
            username: "usname1".to_string(),
            full_name: None,
            birth_date: None,
            phone: None,
            email: None,
            bio: None,
            social_links: None,
            image_uri: None,
        }, AuthType::PASSWORD(Some("pass123".to_string()))).await.expect("user id");


        // let usr1 = LocalUserDbService{ db: &db, ctx: &ctx }.get(IdentIdName::Id(get_string_thing(usr1).unwrap())).await.expect("got user");
        let usr1 = LocalUserDbService{ db: &db, ctx: &ctx }.get(IdentIdName::ColumnIdent{column:"id".to_string(), val:get_string_thing(usr1).unwrap().to_raw(), rec:true}).await.expect("got user");
        dbg!(usr1);
    }

    #[tokio::test]
    async fn prod_balance_0() {

        let (db, ctx) = init_db_test().await;

        let usr1 = LocalUserDbService { db: &db, ctx: &ctx }.create(LocalUser {
            id: None,
            username: "usname1".to_string(),
            full_name: None,
            birth_date: None,
            phone: None,
            email: None,
            bio: None,
            social_links: None,
            image_uri: None,
        }, AuthType::PASSWORD(Some("pass123".to_string()))).await.expect("user");

        let balance_view1 = WalletDbService { db: &db, ctx: &ctx, is_development: false }.get_balance(get_string_thing(usr1.clone()).expect("thing1")).await.expect("balance");
        // dbg!(&balance_view1);
        assert_eq!(balance_view1.user_id.clone().to_raw(), usr1.clone().as_str());
        assert_eq!(balance_view1.balance, 0);
    }

    #[tokio::test]
    async fn make_balance_tx() {
        let (db, ctx) = init_db_test().await;

        let usr1 = LocalUserDbService { db: &db, ctx: &ctx }.create(LocalUser {
            id: None,
            username: "usname1".to_string(),
            full_name: None,
            birth_date: None,
            phone: None,
            email: None,
            bio: None,
            social_links: None,
            image_uri: None,
        }, AuthType::PASSWORD(Some("pass123".to_string()))).await.expect("user");

        let usr2 = LocalUserDbService { db: &db, ctx: &ctx }.create(LocalUser {
            id: None,
            username: "usname2".to_string(),
            full_name: None,
            birth_date: None,
            phone: None,
            email: None,
            bio: None,
            social_links: None,
            image_uri: None,
        }, AuthType::PASSWORD(Some("pass234".to_string()))).await.expect("user2");


        let balance_view1 = WalletDbService { db: &db, ctx: &ctx, is_development: true }.get_balance(get_string_thing(usr1.clone()).expect("thing1")).await.expect("balance");
        // dbg!(&balance_view1);
        assert_eq!(balance_view1.user_id.clone().to_raw(), usr1.clone().as_str());
        assert_eq!(balance_view1.balance, 100);

        let balance_view2 = WalletDbService { db: &db, ctx: &ctx, is_development: true }.get_balance(get_string_thing(usr2.clone()).expect("thing2")).await.expect("balance");
        // dbg!(&balance_view2)
        assert_eq!(balance_view2.user_id.clone().to_raw(), usr2.clone().as_str());
        assert_eq!(balance_view2.balance, 100);

        // backup(db).await;

        let moved = CurrencyTransactionDbService { db: &db, ctx: &ctx }.move_amount(balance_view1.id, balance_view2.id, 100, CurrencySymbol::USD).await; //.expect("move balance");
        // dbg!(moved).expect("dbg");

        let balance_view1 = WalletDbService { db: &db, ctx: &ctx, is_development: true }.get_balance(get_string_thing(usr1.clone()).expect("thing1")).await.expect("balance");
        dbg!(&balance_view1);
        assert_eq!(balance_view1.user_id.clone().to_raw(), usr1.clone().as_str());
        assert_eq!(balance_view1.balance, 0);

        let balance_view2 = WalletDbService { db: &db, ctx: &ctx, is_development: true }.get_balance(get_string_thing(usr2.clone()).expect("thing2")).await.expect("balance");
        dbg!(&balance_view2);
        assert_eq!(balance_view2.user_id.clone().to_raw(), usr2.clone().as_str());
        assert_eq!(balance_view2.balance, 200);

        let moved = CurrencyTransactionDbService { db: &db, ctx: &ctx }.move_amount(balance_view1.id, balance_view2.id, 1, CurrencySymbol::USD).await; //.expect("move balance");
        assert_eq!(moved.is_err(), true);
    }


    // derive Display only stringifies enum ident, serde also serializes the value
    #[derive(Debug, PartialEq,  Serialize, Deserialize, Display)]
    pub enum SomeTestEnum {
            UserFollowAdded{username: String, rec: Thing, opt: Option<String>},
            UserTaskRequestComplete{task_id: String, deliverables:Vec<String>},
        }

    #[derive(Serialize, Deserialize, Debug)]
    struct Val{
        id: Option<Thing>,
        value: SomeTestEnum
    }

    #[tokio::test]
    async fn test_enum_field_literal(){

        let (db, ctx) = init_db_test().await;
        let qry = r#"DEFINE TABLE test_enum SCHEMAFULL;
    DEFINE FIELD value ON TABLE test_enum TYPE {UserFollowAdded:{username:string, rec: record, opt: option<string>}} | {UserTaskRequestComplete:{task_id: string, deliverables:array<string>}};"#;

        &db.query(qry).await.expect("table defined");

        let s=serde_json::to_string(&SomeTestEnum::UserFollowAdded { username: "usss".to_string(), rec: Thing::from(("test_enum", "32432fa")), opt: Some("vall".to_string()) }).expect("string");
        println!("hhh={}",s);

        let uuu: SomeTestEnum = serde_json::from_str(s.as_str()).expect("back");
        dbg!(&uuu);

        println!("{}", &uuu.to_string());
        // derive Display only stringifies enum ident, serde also serializes the value
        assert_eq!("UserFollowAdded", &uuu.to_string());

        let res: Option<Val> = db.create("test_enum").content(Val{ id: None, value:uuu}).await.expect("saved");
        dbg!(&res);
        let res:Option<Val> = db.select(("test_enum", res.unwrap().id.unwrap().id.to_raw())).await.expect("rec");
        dbg!(res);

        let res: Option<Val> = db.create("test_enum").content(Val{ id: None, value:SomeTestEnum::UserTaskRequestComplete {
            task_id: "taaask:123".to_string(),
            deliverables: vec!["one".to_string()],
        }}).await.expect("saved");
        dbg!(&res);
        let res:Option<Val> = db.select(("test_enum", res.unwrap().id.unwrap().id.to_raw())).await.expect("rec");
        dbg!(res);

    }

    async fn backup(_db: db::Db) {
        let mut backup = _db.export(()).await.unwrap();
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open("/Users/mac02/dev/DB_BACKUP.surql")
            .await
            .unwrap();
        // println!("DB BBACC={:?}", file.metadata().unwrap());
        while let Some(result) = backup.next().await {
            match result {
                Ok(bytes) => {
                    file.write_all(bytes.as_slice()).await.unwrap();
                }
                Err(error) => {
                    // Handle the export error
                    println!("ERRRRRR {}", error);
                }
            }
        }
    }

    async fn runMigrations(db: Surreal<Db>) -> AppResult<()> {
        let c = Ctx::new(Ok("migrations".parse().unwrap()), Uuid::new_v4(), false);

        LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
        WalletDbService { db: &db, ctx: &c, is_development: true }.mutate_db().await?;
        CurrencyTransactionDbService { db: &db, ctx: &c }.mutate_db().await?;
        Ok(())
    }

    async fn init_db_test() -> (Surreal<Db>, Ctx) {
        let db = db::start(None).await.expect("db initialized");
        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);

        runMigrations(db.clone()).await.expect("init migrations");
        (db, ctx)
    }
}
