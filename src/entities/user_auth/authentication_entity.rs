use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use surrealdb::types::{RecordId, SurrealValue};

use crate::middleware;
use crate::{
    database::client::Db, entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

#[derive(Debug, Serialize, SurrealValue)]
pub struct CreateAuthInput {
    pub local_user: RecordId,
    pub token: String,
    pub auth_type: AuthType,
    pub passkey_json: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, EnumString, SurrealValue)]
pub enum AuthType {
    PASSWORD,
    PASSKEY,
    APPLE,
    FACEBOOK,
    GOOGLE,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
pub struct Authentication {
    pub id: RecordId,
    pub local_user: RecordId,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey_json: Option<String>,
    pub token: String,
    pub updated_at: DateTime<Utc>,
}

const TABLE_NAME: &str = "authentication";

pub struct AuthenticationDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

impl<'a> AuthenticationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS local_user ON TABLE {TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS auth_type ON TABLE {TABLE_NAME} TYPE string;
            DEFINE FIELD IF NOT EXISTS token ON TABLE {TABLE_NAME} TYPE string;
            DEFINE FIELD IF NOT EXISTS passkey_json ON TABLE {TABLE_NAME} TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {TABLE_NAME} TYPE datetime VALUE time::now();

            DEFINE INDEX IF NOT EXISTS local_user_idx ON TABLE {TABLE_NAME} COLUMNS local_user;
            DEFINE INDEX IF NOT EXISTS token_idx ON TABLE {TABLE_NAME} COLUMNS token;
            DEFINE INDEX IF NOT EXISTS auth_type_idx ON TABLE {TABLE_NAME} COLUMNS auth_type;
        ");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate local user");

        Ok(())
    }

    pub async fn create(&self, input: CreateAuthInput) -> CtxResult<Authentication> {
        let create_auth: Option<Authentication> = self.db.create(TABLE_NAME).content(input).await?;
        Ok(create_auth.unwrap())
    }

    pub async fn get_by_auth_type(
        &self,
        user: String,
        auth: AuthType,
    ) -> CtxResult<Option<Authentication>> {
        let user_record = RecordId::parse_simple(&user)
            .unwrap_or_else(|_| RecordId::new(TABLE_NAME, user.as_str()));
        let mut res = self
            .db
            .query("SELECT * FROM type::table($table) WHERE local_user=<record>$user AND auth_type=$auth_type;")
            .bind(("table", TABLE_NAME))
            .bind(("user", user_record))
            .bind(("auth_type", auth))
            .await?;

        Ok(res.take::<Option<Authentication>>(0)?)
    }

    pub async fn get_by_token(
        &self,
        auth: AuthType,
        token: String,
    ) -> CtxResult<Option<Authentication>> {
        let mut res = self
            .db
            .query("SELECT * FROM type::table($table) WHERE token=$value AND auth_type=$auth_type;")
            .bind(("table", TABLE_NAME))
            .bind(("value", token))
            .bind(("auth_type", auth))
            .await?;
        Ok(res.take::<Option<Authentication>>(0)?)
    }

    pub async fn get_by_user(&self, user: RecordId) -> CtxResult<Vec<Authentication>> {
        let mut res = self
            .db
            .query("SELECT * FROM type::table($table) WHERE local_user=<record>$user AND auth_type=$a_type;")
            .bind(("table", TABLE_NAME))
            .bind(("user", user))
            .await?;
        Ok(res
            .take::<Option<Vec<Authentication>>>(0)?
            .unwrap_or_default())
    }

    pub async fn update_token(
        &self,
        user_id: &str,
        auth_type: AuthType,
        token: String,
    ) -> CtxResult<()> {
        let res = self
            .db
            .query(
                "UPDATE type::table($table) SET token=$value  WHERE local_user=<record>$user AND auth_type=$auth_type;",
            )
            .bind(("table", TABLE_NAME))
            .bind(("user", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("auth_type", auth_type))
            .bind(("value", token))
            .await?;

        res.check()?;
        Ok(())
    }
}
