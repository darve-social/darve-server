use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::middleware;
use access_right_entity::AccessRightDbService;
use authorization_entity::Authorization;
use middleware::db;
use middleware::error::AppError::EntityFailIdNotFound;
use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, RecordWithId,
    ViewFieldSelector,
};
use middleware::utils::string_utils::get_string_thing;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

use super::{access_right_entity, authorization_entity};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserCode {
    pub id: Thing,
    pub code: String,
    pub failed_code_attempts: u8,
    pub user: Thing,
    pub email: String,
    pub use_for: UseCodeFor,
    pub r_created: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LocalUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub social_links: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
}

impl LocalUser {
    pub fn default(username: String) -> Self {
        LocalUser {
            id: None,
            username,
            full_name: None,
            birth_date: None,
            phone: None,
            email_verified: None,
            bio: None,
            social_links: None,
            image_uri: None,
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct UsernameView {
    id: Thing,
    username: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UseCodeFor {
    EmailVerification,
    ResetPassword,
}

impl ViewFieldSelector for UsernameView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, username".to_string()
    }
}

pub struct LocalUserDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "local_user";

impl<'a> LocalUserDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
        // EMAIL is already verified  
    DEFINE FIELD IF NOT EXISTS email_verified ON TABLE {TABLE_NAME} TYPE option<string>;// VALUE string::lowercase($value) ASSERT string::is::email($value);
    DEFINE FIELD IF NOT EXISTS username ON TABLE {TABLE_NAME} TYPE string VALUE string::lowercase($value);
    DEFINE FIELD IF NOT EXISTS full_name ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS birth_date ON TABLE {TABLE_NAME} TYPE option<datetime>;
    DEFINE FIELD IF NOT EXISTS phone ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS bio ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS social_links ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS image_uri ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE INDEX IF NOT EXISTS local_user_username_idx ON TABLE {TABLE_NAME} COLUMNS username UNIQUE;
    DEFINE INDEX IF NOT EXISTS local_user_email_verified_idx ON TABLE {TABLE_NAME} COLUMNS email_verified UNIQUE;
    
    DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS class FILTERS lowercase,ascii;
    DEFINE INDEX IF NOT EXISTS username_txt_idx ON TABLE {TABLE_NAME} COLUMNS username SEARCH ANALYZER ascii BM25 HIGHLIGHTS;
    DEFINE INDEX IF NOT EXISTS full_name_txt_idx ON TABLE {TABLE_NAME} COLUMNS full_name SEARCH ANALYZER ascii BM25 HIGHLIGHTS;

    DEFINE TABLE IF NOT EXISTS user_codes SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE user_codes TYPE record<{TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS email ON TABLE user_codes TYPE string;
    DEFINE FIELD IF NOT EXISTS use_for ON TABLE user_codes TYPE string;
    DEFINE FIELD IF NOT EXISTS code ON TABLE user_codes TYPE string;
    DEFINE FIELD IF NOT EXISTS failed_code_attempts ON TABLE user_codes TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE user_codes TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE INDEX IF NOT EXISTS user_idx ON TABLE user_codes COLUMNS user;
    DEFINE INDEX IF NOT EXISTS code_idx ON TABLE user_codes COLUMNS code;
    DEFINE INDEX IF NOT EXISTS use_for_idx ON TABLE user_codes COLUMNS use_for;
");
        let local_user_mutation = self.db.query(sql).await?;

        local_user_mutation
            .check()
            .expect("should mutate local_user");

        Ok(())
    }

    pub async fn get_ctx_user_id(&self) -> CtxResult<String> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        let existing_id = self.exists(IdentIdName::Id(user_id)).await?;
        match existing_id {
            None => Err(self
                .ctx
                .to_ctx_error(EntityFailIdNotFound { ident: created_by })),
            Some(uid) => Ok(uid),
        }
    }

    pub async fn get_ctx_user_thing(&self) -> CtxResult<Thing> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        let existing_id = self.exists(IdentIdName::Id(user_id.clone())).await?;
        match existing_id {
            None => Err(self
                .ctx
                .to_ctx_error(EntityFailIdNotFound { ident: created_by })),
            Some(_uid) => Ok(user_id),
        }
    }

    pub async fn is_ctx_user_authorised(&self, authorization: &Authorization) -> CtxResult<()> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        AccessRightDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .is_authorized(&user_id, authorization)
        .await
    }

    pub async fn get_ctx_user(&self) -> CtxResult<LocalUser> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        self.get(IdentIdName::Id(user_id)).await
    }

    pub async fn exists(&self, ident: IdentIdName) -> CtxResult<Option<String>> {
        exists_entity(self.db, TABLE_NAME.to_string(), &ident)
            .await
            .map(|r| r.map(|o| o.to_raw()))
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<LocalUser> {
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }
    pub async fn get_by_email(&self, email: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::ColumnIdent {
            column: "email_verified".to_string(),
            val: email.to_string(),
            rec: false,
        };
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_username(&self, ident: IdentIdName) -> CtxResult<String> {
        let u_view = self.get_view::<UsernameView>(ident).await?;
        Ok(u_view.username)
    }

    pub async fn search(&self, find: String) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT id, username, full_name, image_uri FROM {TABLE_NAME} WHERE username ~ $find OR full_name ~ $find;");
        let res = self.db.query(qry).bind(("find", find));
        let res: Vec<LocalUser> = res.await?.take(0)?;
        Ok(res)
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn create(&self, ct_input: LocalUser) -> CtxResult<String> {
        let local_user_id: String = self
            .db
            .create(TABLE_NAME)
            .content(ct_input)
            .await
            .map(|v: Option<RecordWithId>| v.unwrap().id.id.to_raw())
            .map(|id| format!("{TABLE_NAME}:{id}"))
            .map_err(CtxError::from(self.ctx))?;
        Ok(local_user_id)
    }

    pub async fn update(&self, record: LocalUser) -> CtxResult<LocalUser> {
        let resource = record.id.clone().ok_or(AppError::Generic {
            description: "can not update user with no id".to_string(),
        })?;
        // record.r_created = None;

        let disc_topic: Option<LocalUser> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(disc_topic.unwrap())
    }

    pub async fn users_len(&self) -> CtxResult<i32> {
        let q = format!("SELECT count() FROM {TABLE_NAME} limit 1");
        let res: Option<i32> = self.db.query(q).await?.take("count")?;
        Ok(res.unwrap_or(0))
    }

    pub async fn get_code(
        &self,
        user_id: Thing,
        use_for: UseCodeFor,
    ) -> CtxResult<Option<UserCode>> {
        let qry = "SELECT * FROM user_codes WHERE user = $user_id AND use_for = $use_for;";
        let mut res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("use_for", use_for))
            .await?;
        let data: Option<UserCode> = res.take(0)?;
        Ok(data)
    }

    pub async fn create_code(
        &self,
        user_id: Thing,
        code: String,
        email: String,
        use_for: UseCodeFor,
    ) -> CtxResult<()> {
        let qry = "
            BEGIN TRANSACTION;
                DELETE FROM user_codes WHERE user = $user_id AND use_for = $use_for;
                CREATE user_codes SET user=$user_id, code=$code, email=$email, use_for=$use_for;
            COMMIT TRANSACTION;
        ";
        let res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("code", code))
            .bind(("email", email))
            .bind(("use_for", use_for))
            .await?;
        res.check()?;
        Ok(())
    }

    pub async fn increase_code_attempt(&self, code_id: Thing) -> CtxResult<()> {
        let res = self
            .db
            .query("UPDATE $code_id SET failed_code_attempts += 1;")
            .bind(("code_id", code_id.clone()))
            .await?;
        res.check()?;
        Ok(())
    }

    pub async fn delete_code(&self, id: Thing) -> CtxResult<()> {
        let _: Option<UserCode> = self.db.delete((id.tb, id.id.to_raw())).await?;
        Ok(())
    }

    pub async fn set_user_email(&self, user_id: Thing, verified_email: String) -> CtxResult<()> {
        let qry = "
            BEGIN TRANSACTION;
                DELETE FROM user_codes WHERE user = $user_id AND use_for = $use_for;
                UPDATE $user_id SET email_verified = $email;
            COMMIT TRANSACTION;
        ";
        let res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("email", verified_email))
            .bind(("use_for", UseCodeFor::EmailVerification))
            .await?;

        res.check()?;

        Ok(())
    }
}
