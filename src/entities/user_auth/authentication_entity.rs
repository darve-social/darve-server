use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::EnumString;
use surrealdb::sql::Thing;
use webauthn_rs::prelude::{CredentialID, Passkey};

use middleware::db;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

use crate::middleware;
use crate::middleware::error::AppResult;
use crate::middleware::utils::db_utils::QryBindingsVal;
use crate::middleware::utils::string_utils::get_string_thing;

#[derive(Clone, Debug, Serialize, Deserialize, EnumString)]
pub enum AuthType {
    PASSWORD(Option<String>, Option<Thing>), //  password, user_id
    PASSKEY(Option<CredentialID>, Option<Passkey>),
    APPLE(String),    //  external apple user id
    FACEBOOK(String), //  external facebook user id
    GOOGLE(String),   //  external google user id
}

impl AuthType {
    pub fn type_str(&self) -> &'static str {
        match self.clone() {
            AuthType::PASSWORD(_, _) => "PASSWORD",
            AuthType::PASSKEY(_, _) => "PASSKEY",
            AuthType::APPLE(_) => "APPLE",
            AuthType::FACEBOOK(_) => "FACEBOOK",
            AuthType::GOOGLE(_) => "GOOGLE",
        }
    }

    pub fn identifier_val(&self) -> Option<String> {
        match self.clone() {
            AuthType::PASSWORD(pass, user_id) => match (pass, user_id) {
                // TODO -hash password- https://docs.rs/argon2/0.5.3/argon2/
                // when creating hash use both pass+user_id concatenated
                (Some(pass), Some(user_id)) => Some(format!("{}{}", pass.clone(), user_id.clone())),
                _ => None,
            },
            AuthType::PASSKEY(passkey_cred_id, paksskey) => {
                let mut cid = match passkey_cred_id {
                    None => None,
                    Some(passkey_cid) => Some(passkey_cid.clone()),
                };
                if cid.is_none() {
                    cid = match paksskey {
                        None => None,
                        Some(pk) => Some(pk.clone().cred_id().clone()),
                    }
                }
                match cid {
                    None => None,
                    Some(cid) => Some(STANDARD.encode(cid.to_vec())),
                }
            }
            AuthType::APPLE(user_id) => Some(user_id),
            AuthType::FACEBOOK(user_id) => Some(user_id),
            AuthType::GOOGLE(user_id) => Some(user_id),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authentication {
    pub id: Thing, // AuthenticationId,
    pub local_user: Thing,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey_json: Option<String>,
    pub updated: Option<String>,
}

impl Authentication {
    pub fn new(user_id: String, auth: AuthType) -> Result<Authentication, AppError> {
        let passkey_json = match auth.clone() {
            AuthType::PASSKEY(_, pk) => {
                let str_res = serde_json::to_string(&pk);
                if str_res.is_err() {
                    return Err(str_res
                        .map_err(|e| AppError::Serde {
                            source: e.to_string(),
                        })
                        .unwrap_err());
                }
                str_res.ok()
            }
            _ => None,
        };

        let id = Self::get_string_id(TABLE_NAME.to_string(), user_id.clone(), auth.clone())?;
        Ok(Authentication {
            id: id.parse().unwrap(),
            local_user: user_id.parse().unwrap(),
            auth_type: auth.type_str().to_string(),
            passkey_json,
            updated: None,
        })
    }

    pub fn get_string_id(
        table: String,
        local_user_id: String,
        auth_type: AuthType,
    ) -> AppResult<String> {
        let mut a_type = auth_type.clone();
        if let AuthType::PASSWORD(Some(pass), None) = auth_type {
            a_type = AuthType::PASSWORD(Some(pass), Some(get_string_thing(local_user_id.clone())?))
        }
        let type_str = a_type.type_str();
        let value = a_type.identifier_val().ok_or(AppError::Generic {
            description: "Authentication value error".to_string(),
        })?;
        Ok(format!(
            "{table}:['{value}','{type_str}','{local_user_id}']"
        ))
    }
}

const TABLE_NAME: &str = "authentication";

pub struct AuthenticationDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> AuthenticationDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!(
            "
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS local_user ON TABLE {TABLE_NAME} TYPE record<local_user>;
    DEFINE INDEX IF NOT EXISTS local_user_idx ON TABLE {TABLE_NAME} COLUMNS local_user;
    DEFINE FIELD IF NOT EXISTS auth_type ON TABLE {TABLE_NAME} TYPE string VALUE string::uppercase($value)
        ASSERT $value INSIDE ['PASSWORD','EMAIL','PUBLIC_KEY','PASSKEY', 'APPLE', 'FACEBOOK', 'GOOGLE'];
    DEFINE FIELD IF NOT EXISTS passkey_json ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS updated ON TABLE {TABLE_NAME} TYPE datetime VALUE time::now();
    "
        );
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate local user");

        Ok(())
    }

    pub async fn create(&self, auth_input: Authentication) -> CtxResult<bool> {
        let create_auth: Option<Authentication> =
            self.db.create(TABLE_NAME).content(auth_input).await?;
        Ok(create_auth.is_some())
    }

    pub async fn authenticate(&self, ctx: &Ctx, auth: AuthType) -> CtxResult<String> {
        let q = match auth.clone() {
            AuthType::PASSWORD(_pass, _user_id) => {
             QryBindingsVal::new("SELECT id, local_user FROM <record>authentication:[$external_ident, $auth_type, NONE]..=[$external_ident,$auth_type,..];".to_string(), HashMap::from([
                    ("external_ident".to_string(), auth
                        .identifier_val()
                        .ok_or(
                            ctx.to_ctx_error(AppError::Generic {description:"Auth type value missing".to_string()})
                        )?),
                    ("auth_type".to_string(), auth.type_str().to_string())
                ]))
            }
            _=>{
                return Err(ctx.to_ctx_error(AppError::Generic {description:format!("Authentication not implemented yet for {}", auth.type_str())}));
            }
        };
        let mut select_authentication = q.into_query(self.db).await?;
        let rec_found: Option<Thing> = select_authentication.take("local_user")?;
        match rec_found {
            None => Err(ctx.to_ctx_error(AppError::AuthenticationFail {})),
            Some(user_id) => Ok(user_id.to_raw()),
        }
    }

    pub async fn get_by_auth_type(
        &self,
        local_user_id: String,
        auth_type: AuthType,
    ) -> CtxResult<Vec<Authentication>> {
        let a_type = auth_type.type_str();
        let q = "SELECT * FROM type::table($table) WHERE local_user=<record>$local_user_id AND auth_type=$a_type;".to_string();
        let res = self
            .db
            .query(q)
            .bind(("table", TABLE_NAME))
            .bind(("local_user_id", local_user_id))
            .bind(("a_type", a_type))
            .await;
        match res {
            Ok(response) => {
                let mut response = response;
                let rec = response.take(0)?;
                Ok(rec)
            }
            Err(e) => {
                dbg!(e);
                Ok(vec![])
            }
        }
    }
}
