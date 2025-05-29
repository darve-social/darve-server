use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::middleware;
use access_right_entity::AccessRightDbService;
use authentication_entity::{AuthType, Authentication, AuthenticationDbService};
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

use super::{access_right_entity, authentication_entity, authorization_entity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailVerification {
    pub code: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
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
    pub email: Option<String>,
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
            email: None,
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
    DEFINE FIELD IF NOT EXISTS email ON TABLE {TABLE_NAME} TYPE option<string>;// VALUE string::lowercase($value) ASSERT string::is::email($value);
    DEFINE FIELD IF NOT EXISTS username ON TABLE {TABLE_NAME} TYPE string VALUE string::lowercase($value);
    DEFINE FIELD IF NOT EXISTS full_name ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS birth_date ON TABLE {TABLE_NAME} TYPE option<datetime>;
    DEFINE FIELD IF NOT EXISTS phone ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS bio ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS social_links ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS image_uri ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE INDEX IF NOT EXISTS local_user_username_idx ON TABLE {TABLE_NAME} COLUMNS username UNIQUE;
    DEFINE INDEX IF NOT EXISTS local_user_email_idx ON TABLE {TABLE_NAME} COLUMNS email UNIQUE;
    
    DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS class FILTERS lowercase,ascii;
    DEFINE INDEX IF NOT EXISTS username_txt_idx ON TABLE {TABLE_NAME} COLUMNS username SEARCH ANALYZER ascii BM25 HIGHLIGHTS;
    DEFINE INDEX IF NOT EXISTS full_name_txt_idx ON TABLE {TABLE_NAME} COLUMNS full_name SEARCH ANALYZER ascii BM25 HIGHLIGHTS;

    DEFINE TABLE IF NOT EXISTS email_verification SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS user ON TABLE email_verification TYPE record<{TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS email ON TABLE email_verification TYPE string;
    DEFINE FIELD IF NOT EXISTS code ON TABLE email_verification TYPE string;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE email_verification TYPE datetime DEFAULT time::now();
    DEFINE INDEX IF NOT EXISTS user_idx ON TABLE email_verification COLUMNS user UNIQUE;
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

    pub async fn create(&self, ct_input: LocalUser, auth: AuthType) -> CtxResult<String> {
        let local_user_id: String = self
            .db
            .create(TABLE_NAME)
            .content(ct_input)
            .await
            .map(|v: Option<RecordWithId>| v.unwrap().id.id.to_raw())
            .map(|id| format!("{TABLE_NAME}:{id}"))
            .map_err(CtxError::from(self.ctx))?;
        let auth = Authentication::new(local_user_id.clone(), auth)?;
        // dbg!(&auth);
        AuthenticationDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .create(auth)
        .await?;
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

    pub async fn get_email_verification(
        &self,
        user_id: Thing,
    ) -> CtxResult<Option<EmailVerification>> {
        let qry = "SELECT * FROM email_verification WHERE user = $user_id";
        let mut res = self.db.query(qry).bind(("user_id", user_id)).await?;
        let data: Option<EmailVerification> = res.take(0)?;
        Ok(data)
    }

    pub async fn create_email_verification(
        &self,
        user_id: Thing,
        code: String,
        email: String,
    ) -> CtxResult<()> {
        let qry = "
            BEGIN TRANSACTION;
                DELETE FROM email_verification WHERE user = $user_id;
                CREATE email_verification SET user = $user_id, code = $code, email = $email;
            COMMIT TRANSACTION;
        ";
        let res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("code", code))
            .bind(("email", email))
            .await?;
        res.check()?;
        Ok(())
    }

    pub async fn update_email(&self, user_id: Thing, email: String) -> CtxResult<()> {
        let qry = "
            BEGIN TRANSACTION;
                DELETE FROM email_verification WHERE user = $user_id;
                UPDATE $user_id SET email = $email;
            COMMIT TRANSACTION;
        ";
        let res = self
            .db
            .query(qry)
            .bind(("user_id", user_id))
            .bind(("email", email))
            .await?;

        res.check()?;

        Ok(())
    }
}
