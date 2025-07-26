use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::{access_right_entity, authorization_entity};
use crate::database::client::Db;
use crate::database::repositories::verification_code_repo::VERIFICATION_CODE_TABLE_NAME;
use crate::database::surrdb_utils::get_str_id_thing;
use crate::entities::verification_code::VerificationCodeFor;
use crate::middleware;
use access_right_entity::AccessRightDbService;
use authorization_entity::Authorization;
use middleware::error::AppError::EntityFailIdNotFound;
use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::utils::string_utils::get_string_thing;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LocalUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,
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

impl ViewFieldSelector for UsernameView {
    fn get_select_query_fields() -> String {
        "id, username".to_string()
    }
}

pub struct LocalUserDbService<'a> {
    pub db: &'a Db,
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
    DEFINE FIELD IF NOT EXISTS birth_date ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS phone ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS bio ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS social_links ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS image_uri ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE INDEX IF NOT EXISTS local_user_username_idx ON TABLE {TABLE_NAME} COLUMNS username UNIQUE;
    DEFINE INDEX IF NOT EXISTS local_user_email_verified_idx ON TABLE {TABLE_NAME} COLUMNS email_verified UNIQUE;
  
    DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS class FILTERS lowercase,ascii;
    DEFINE INDEX IF NOT EXISTS username_txt_idx ON TABLE {TABLE_NAME} COLUMNS username SEARCH ANALYZER ascii BM25 HIGHLIGHTS;
    DEFINE INDEX IF NOT EXISTS full_name_txt_idx ON TABLE {TABLE_NAME} COLUMNS full_name SEARCH ANALYZER ascii BM25 HIGHLIGHTS;

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

    // param id is a id of the thing
    pub async fn get_by_id(&self, id: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::Id(get_str_id_thing(TABLE_NAME, id)?);
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_by_email(&self, email: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::ColumnIdent {
            column: "email_verified".to_string(),
            val: email.to_string(),
            rec: false,
        };
        self.get(ident).await
    }

    pub async fn get_by_username(&self, value: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::ColumnIdent {
            column: "username".to_string(),
            val: value.to_string(),
            rec: false,
        };
        self.get(ident).await
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

    // TODO surrealdb datetime issue https://github.com/surrealdb/surrealdb/issues/2454
    pub async fn create(&self, ct_input: LocalUser) -> CtxResult<String> {
        let user: Option<LocalUser> = self.db.create(TABLE_NAME).content(ct_input).await?;
        Ok(user.unwrap().id.as_ref().unwrap().to_raw())
    }

    pub async fn update(&self, record: LocalUser) -> CtxResult<LocalUser> {
        let resource = record.id.clone().ok_or(AppError::Generic {
            description: "can not update user with no id".to_string(),
        })?;
        let user: Option<LocalUser> = self
            .db
            .update((TABLE_NAME, resource.id.to_raw()))
            .content(record)
            .await?;
        Ok(user.unwrap())
    }

    pub async fn users_len(&self) -> CtxResult<i32> {
        let q = format!("SELECT count() FROM {TABLE_NAME} limit 1");
        let res: Option<i32> = self.db.query(q).await?.take("count")?;
        Ok(res.unwrap_or(0))
    }

    pub async fn set_user_email(&self, user_id: Thing, verified_email: String) -> CtxResult<()> {
        let qry = format!(
            "
            BEGIN TRANSACTION;
                DELETE FROM {VERIFICATION_CODE_TABLE_NAME} WHERE user = $user_id AND use_for = $use_for;
                UPDATE $user_id SET email_verified = $email;
            COMMIT TRANSACTION;
        "
        );
        let res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("email", verified_email))
            .bind(("use_for", VerificationCodeFor::EmailVerification))
            .await?;

        res.check()?;

        Ok(())
    }
}
