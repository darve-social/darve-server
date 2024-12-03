use std::ops::{Add, Deref};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::opt::QueryResult;
use surrealdb::opt::IntoResource;
use surrealdb::sql::Thing;

use crate::entity::authentication_entity::{AuthType, Authentication, AuthenticationDbService};
use crate::entity::authorization_entity::Authorization;
use sb_middleware::db;
use sb_middleware::error::AppError::EntityFailIdNotFound;
use sb_middleware::utils::db_utils::{exists_entity, get_entity, get_entity_view, record_exists, with_not_found_err, IdentIdName, RecordWithId, ViewFieldSelector};
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};
use sb_middleware::utils::string_utils::get_string_thing;
use crate::entity::access_right_entity::AccessRightDbService;

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
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD email ON TABLE {TABLE_NAME} TYPE option<string>;// VALUE string::lowercase($value) ASSERT string::is::email($value);
    DEFINE FIELD username ON TABLE {TABLE_NAME} TYPE string VALUE string::lowercase($value);
    DEFINE FIELD full_name ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD birth_date ON TABLE {TABLE_NAME} TYPE option<datetime>;
    DEFINE FIELD phone ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD bio ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD social_links ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD image_uri ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE INDEX local_user_username_idx ON TABLE {TABLE_NAME} COLUMNS username UNIQUE;
    DEFINE INDEX local_user_email_idx ON TABLE {TABLE_NAME} COLUMNS email UNIQUE;
");
        let local_user_mutation = self.db
            .query(sql)
            .await?;

        &local_user_mutation.check().expect("should mutate local_user");

        Ok(())
    }

    pub async fn get_ctx_user_id(&self) -> CtxResult<String> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        let existing_id = self.exists(IdentIdName::Id(user_id)).await?;
        match existing_id {
            None => Err(self.ctx.to_ctx_error(EntityFailIdNotFound { ident: created_by })),
            Some(uid) => Ok(uid)
        }
    }

    pub async fn get_ctx_user_thing(&self) -> CtxResult<Thing> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        let existing_id = self.exists(IdentIdName::Id(user_id.clone())).await?;
        match existing_id {
            None => Err(self.ctx.to_ctx_error(EntityFailIdNotFound { ident: created_by })),
            Some(_uid) => Ok(user_id)
        }
    }

    pub async fn is_ctx_user_authorised(&self, authorization: &Authorization) -> CtxResult<()> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        AccessRightDbService{ db: self.db, ctx: self.ctx }.is_authorized(&user_id, authorization).await
    }

    pub async fn get_ctx_user(&self) -> CtxResult<LocalUser> {
        let created_by = self.ctx.user_id()?;
        let user_id = get_string_thing(created_by.clone())?;
        self.get(IdentIdName::Id(user_id)).await
    }

    pub async fn exists(&self, ident: IdentIdName) -> CtxResult<Option<String>> {
        exists_entity(self.db, TABLE_NAME.to_string(), &ident).await.map(|r| r.map(|o| o.to_raw()))
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<LocalUser> {
        let opt = get_entity::<LocalUser>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_username(&self, ident: IdentIdName) -> CtxResult<String> {
        let u_view = self.get_view::<UsernameView>(ident).await?;
        Ok(u_view.username)
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, ident_id_name: IdentIdName) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_user_by_username(&self, username: &str) -> CtxResult<LocalUser> {
        let ident = IdentIdName::ColumnIdent {
            column: "username".to_string(),
            val: username.to_string(),
            rec: false,
        };
        let user = self.get(ident).await?;

        Ok(user)
    }

    pub async fn create(&self, ct_input: LocalUser, auth: AuthType) -> CtxResult<String> {
        let local_user_id: String = self.db
            .create(TABLE_NAME)
            .content(ct_input)
            .await
            .map(|v: Option<RecordWithId>| v.unwrap().id.id.to_raw())
            .map(|id| format!("{TABLE_NAME}:{id}"))
            .map_err(CtxError::from(self.ctx))?;
        let auth = Authentication::new(local_user_id.clone(), auth)?;
        // dbg!(&auth);
        AuthenticationDbService { db: self.db, ctx: self.ctx }.create(auth).await?;
        Ok(local_user_id)
    }

    pub async fn update(&self, mut record: LocalUser) -> CtxResult<LocalUser> {
        let resource = record.id.clone().ok_or(AppError::Generic { description: "can not update user with no id".to_string() })?;
        // record.r_created = None;

        let disc_topic: Option<LocalUser> = self.db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(disc_topic.unwrap())
    }

    pub async fn users_len(&self) -> CtxResult<i32> {
        let q = format!("SELECT count() FROM {TABLE_NAME}");
        let res: Option<i32> = self.db.query(q).await?.take("count")?;
        Ok(res.unwrap_or(0))
    }
}
