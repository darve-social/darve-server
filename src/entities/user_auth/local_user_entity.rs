use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::database::client::Db;
use crate::database::repositories::verification_code_repo::VERIFICATION_CODE_TABLE_NAME;
use crate::database::surrdb_utils::{get_entity, get_str_id_thing, record_id_to_raw};
use crate::entities::user_auth::authentication_entity::{AuthType, Authentication};
use crate::entities::verification_code::VerificationCodeFor;
use crate::middleware;
use crate::middleware::utils::db_utils::{record_exists, Pagination};
use middleware::error::AppError::EntityFailIdNotFound;
use middleware::utils::db_utils::{
    exists_entity, get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::utils::string_utils::get_string_thing;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, SurrealValue)]
pub enum UserRole {
    Admin,
    User,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
pub struct LocalUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
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
    #[serde(default)]
    pub is_otp_enabled: bool,
    pub otp_secret: Option<String>,
    #[serde(default)]
    pub credits: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,
    pub role: UserRole,
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
            is_otp_enabled: false,
            otp_secret: None,
            credits: 0,
            last_seen: None,
            role: UserRole::User,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct UpdateUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_otp_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_secret: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub social_links: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<Option<String>>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct UsernameView {
    id: RecordId,
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
    DEFINE FIELD IF NOT EXISTS is_otp_enabled ON TABLE {TABLE_NAME} TYPE bool DEFAULT false;
    DEFINE FIELD IF NOT EXISTS otp_secret ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS credits ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS last_seen ON TABLE {TABLE_NAME} TYPE option<datetime>;
    DEFINE FIELD IF NOT EXISTS role ON TABLE {TABLE_NAME} TYPE string;

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

    pub async fn get_ctx_user_thing(&self) -> CtxResult<RecordId> {
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

    pub async fn get_ctx_user(&self) -> CtxResult<LocalUser> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        self.get(IdentIdName::Id(user_id)).await
    }

    pub async fn exists(&self, ident: IdentIdName) -> CtxResult<Option<String>> {
        exists_entity(self.db, TABLE_NAME.to_string(), &ident)
            .await
            .map(|r| r.map(|o| record_id_to_raw(&o)))
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<LocalUser> {
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME, &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_by_id_with_auth(
        &self,
        user_id: &str,
        auth_type: AuthType,
    ) -> CtxResult<(LocalUser, Option<Authentication>)> {
        let mut res  = self.db.query("LET $u = SELECT * FROM ONLY $user; LET $a = IF $u != NONE THEN (SELECT * FROM authentication WHERE local_user=$u.id AND auth_type=$auth_type LIMIT 1)[0] ELSE NONE END; RETURN { user: $u, auth: $a };")
            .bind(("user", RecordId::new(TABLE_NAME, user_id))).bind(("auth_type", auth_type )).await?;

        let user = res
            .take::<Option<LocalUser>>((res.num_statements() - 1, "user"))?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: user_id.to_string(),
            })?;
        let auth = res.take::<Option<Authentication>>((res.num_statements() - 1, "auth"))?;

        Ok((user, auth))
    }

    pub async fn get_by_email_with_auth(
        &self,
        email: &str,
        auth_type: AuthType,
    ) -> CtxResult<(LocalUser, Option<Authentication>)> {
        let mut res  = self.db.query("LET $u = SELECT * FROM ONLY local_user WHERE email_verified=$email; LET $a = IF $u != NONE THEN (SELECT * FROM authentication WHERE local_user=$u.id AND auth_type=$auth_type LIMIT 1)[0] ELSE NONE END; RETURN { user: $u, auth: $a};")
            .bind(("email", email.to_string())).bind(("auth_type", auth_type )).await?;

        let user = res
            .take::<Option<LocalUser>>((res.num_statements() - 1, "user"))?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: email.to_string(),
            })?;
        let auth = res.take::<Option<Authentication>>((res.num_statements() - 1, "auth"))?;

        Ok((user, auth))
    }

    pub async fn get_by_username_with_auth(
        &self,
        username: &str,
        auth_type: AuthType,
    ) -> CtxResult<(LocalUser, Option<Authentication>)> {
        let mut res  = self.db.query("LET $u = SELECT * FROM ONLY local_user WHERE username=$username; LET $a = IF $u != NONE THEN (SELECT * FROM authentication WHERE local_user=$u.id AND auth_type=$auth_type LIMIT 1)[0] ELSE NONE END; RETURN { user: $u, auth: $a };")
            .bind(("username", username.to_string())).bind(("auth_type", auth_type )).await?;

        let user = res
            .take::<Option<LocalUser>>((res.num_statements() - 1, "user"))?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: username.to_string(),
            })?;

        let auth = res.take::<Option<Authentication>>((res.num_statements() - 1, "auth"))?;

        Ok((user, auth))
    }

    // param id is a id of the thing
    pub async fn get_by_id(&self, id: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::Id(RecordId::new(TABLE_NAME, id));
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME, &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    // param id is a id of the thing
    pub async fn exists_by_id(&self, id: &str) -> CtxResult<()> {
        Ok(record_exists(self.db, &get_str_id_thing(TABLE_NAME, id)?).await?)
    }

    pub async fn get_by_ids(&self, ids: Vec<RecordId>) -> CtxResult<Vec<LocalUser>> {
        let mut res = self
            .db
            .query(format!("SELECT * FROM {TABLE_NAME} WHERE id IN $users;"))
            .bind(("users", ids))
            .await?;
        let data = res.take::<Vec<LocalUser>>(0)?;
        Ok(data)
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

    pub async fn get_by_role(&self, role: UserRole) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT * FROM {TABLE_NAME} WHERE role=$role");
        let mut res = self.db.query(qry).bind(("role", role)).await?;
        let data = res.take::<Vec<LocalUser>>(0)?;
        Ok(data)
    }

    pub async fn search<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        find: String,
        pag: Pagination,
    ) -> CtxResult<Vec<T>> {
        let field = T::get_select_query_fields();
        let qry = format!(
            "SELECT {} FROM {TABLE_NAME} WHERE username ~ $find OR full_name ~ $find
             LIMIT $limit START $start;",
            field
        );
        let res = self
            .db
            .query(qry)
            .bind(("find", find))
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .await?
            .take::<Vec<T>>(0)?;

        Ok(res)
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    // TODO surrealdb datetime issue https://github.com/surrealdb/surrealdb/issues/2454
    pub async fn create(&self, ct_input: LocalUser) -> CtxResult<LocalUser> {
        let user: Option<LocalUser> = self.db.create(TABLE_NAME).content(ct_input).await?;
        Ok(user.unwrap())
    }

    pub async fn update(&self, user_id: &str, record: UpdateUser) -> CtxResult<LocalUser> {
        let user: Option<LocalUser> = self.db.update((TABLE_NAME, user_id)).merge(record).await?;
        Ok(user.unwrap())
    }

    pub async fn add_credits(&self, user: RecordId, value: u16) -> CtxResult<u64> {
        let mut res = self
            .db
            .query("UPDATE $user SET credits += $credits RETURN credits;")
            .bind(("credits", value))
            .bind(("user", user))
            .await?;
        let data = res.take::<Option<u64>>((0, "credits"))?;
        Ok(data.unwrap())
    }

    pub async fn remove_credits(&self, user: RecordId, value: u16) -> CtxResult<u64> {
        let mut res = self
            .db
            .query("UPDATE $user SET credits = math::max([0, credits-$credits]) RETURN credits;")
            .bind(("credits", value))
            .bind(("user", user))
            .await?;
        let data = res.take::<Option<u64>>((0, "credits"))?;
        Ok(data.unwrap())
    }

    pub async fn update_last_seen(&self, user_id: &str) -> CtxResult<()> {
        let _ = self
            .db
            .query("UPDATE $user SET last_seen=time::now();")
            .bind(("user", RecordId::new(TABLE_NAME, user_id)))
            .await?
            .check()?;
        Ok(())
    }

    pub async fn users_len(&self) -> CtxResult<i32> {
        let q = format!("SELECT count() FROM {TABLE_NAME} limit 1");
        let res: Option<i32> = self.db.query(q).await?.take("count")?;
        Ok(res.unwrap_or(0))
    }

    pub async fn set_user_email(&self, user_id: RecordId, verified_email: String) -> CtxResult<()> {
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
