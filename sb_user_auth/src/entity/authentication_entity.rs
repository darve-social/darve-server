use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use surrealdb::sql::Thing;
use webauthn_rs::prelude::{CredentialID, Passkey};

use sb_middleware::db;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

/*pub struct PasskeyCredId(String);

impl From<Passkey> for PasskeyCredId {
    fn from(value: Passkey) -> Self {
        PasskeyCredId(value.cred_id())
    }
}*/

#[derive(Clone, Debug, Serialize, Deserialize, EnumString)]
pub enum AuthType {
    PASSWORD(Option<String>), //  password hash
    EMAIL(Option<String>),    //  password hash
    PASSKEY(Option<CredentialID>, Option<Passkey>),
    PUBLICKEY(Option<String>), // eth account cryptography
}

impl AuthType {
    /*pub fn from_str(s: &str) -> Option<MediaType> {
        match s {
            "text/plain" => Some(MediaType::PlainText),
            "application/json" => Some(MediaType::ApplicationJson),
            _ => None,
        }
    }*/

    pub fn as_str(&self) -> &'static str {
        match self.clone() {
            AuthType::PASSWORD(_) => "PASSWORD",
            AuthType::EMAIL(_) => "EMAIL",
            AuthType::PASSKEY(_, _) => "PASSKEY",
            AuthType::PUBLICKEY(_) => "PUBLIC_KEY",
        }
    }

    pub fn as_val(&self) -> Option<String> {
        match self.clone() {
            AuthType::PASSWORD(pass) => match pass {
                None => None,
                // TODO hash password - https://docs.rs/argon2/0.5.3/argon2/
                Some(pass) => Some(pass.clone()),
            },
            AuthType::EMAIL(email) => email,
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
            AuthType::PUBLICKEY(pub_key) => pub_key.clone(),
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

        let id = Self::get_string_id(TABLE_NAME.to_string(), user_id.clone(), auth.clone(), true)?;
        // println!("IDDDD={}", id);
        Ok(Authentication {
            id: id.parse().unwrap(),
            local_user: user_id.parse().unwrap(),
            auth_type: auth.as_str().to_string(),
            passkey_json,
            updated: None,
        })
        // Authentication{id: Self::createId(userId, auth) , timestamp: "ttt".to_string(), confirmed: None }
    }

    /* pub fn createId( local_user_id: String, authType: AuthType)-> Option<Thing> {
        let mut ident = BTreeMap::new();
        ident.insert("local_user_id".to_string(),  StrandVal(Strand(local_user_id.clone())));
        ident.insert("type".to_string(), StrandVal(Strand(authType.clone().as_str().to_string())));
        ident.insert("value".to_string(), StrandVal(Strand(authType.clone().as_val().to_string())));
        //todo timestamp
        let id = Id::from(Object(ident));
       //println!("JSONNNN = {}", serde_json::to_string(&AuthenticationId { local_user_id, auth: authType }).unwrap());
        Some(Thing{tb: TABLE_NAME.to_string(), id: id })
    }*/

    pub fn get_string_id(
        table: String,
        local_user_id: String,
        auth_type: AuthType,
        id_val_required: bool,
    ) -> CtxResult<String> {
        let has_required_params = match auth_type.clone() {
            AuthType::PASSWORD(v) | AuthType::EMAIL(v) | AuthType::PUBLICKEY(v) => {
                v.is_some() && id_val_required || v.is_none()
            }

            AuthType::PASSKEY(pk_cid, pk) => {
                ((pk_cid.is_some() && id_val_required) || (pk.is_some() && id_val_required))
                    || !id_val_required
            }
        };
        if has_required_params == false {
            return Err(CtxError {
                error: AppError::Generic {
                    description: "Authentication id has no required value".parse().unwrap(),
                },
                req_id: Default::default(),
                is_htmx: false,
            });
        }

        let type_str = auth_type.as_str();
        let value = auth_type.as_val();
        match value {
            None => Ok(format!("{table}:[{local_user_id},'{type_str}']")),
            Some(val) => Ok(format!("{table}:[{local_user_id},'{type_str}','{val}']")),
        } /*match value {
              None => Ok(format!("{table}:{{local_user_id:{local_user_id}, type:'{typeStr}', value: None}}")),
              Some(val) =>Ok(format!("{table}:{{local_user_id:{local_user_id}, type:'{typeStr}', value:'{val}'}}"))
          }*/
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
        ASSERT $value INSIDE ['PASSWORD','EMAIL','PUBLIC_KEY','PASSKEY'];
    DEFINE FIELD IF NOT EXISTS passkey_json ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS updated ON TABLE {TABLE_NAME} TYPE datetime VALUE time::now();
    "
        );
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate local user");

        /*let sql1 = "CREATE local_user:usn1 SET username = 'username1';";
        let mut result1 = self.db.query(sql1).await.unwrap();
        let rrrrrrr1: Option<LocalUser> = result1.take(0)?;
        dbg!(rrrrrrr1);
        println!("EEEE {}", self.exists("username1".to_string()).await?.to_string());

        let sql2 = "CREATE local_user:usn2 SET username = 'username1';";
        let mut result2 = self.db.query(sql2).await?;
        // let mut result2 = self.db.query(sql2).await.unwrap().check().is_err();
        let rrrrrrr2: Option<LocalUser> = result2.take(0)?;
        dbg!(rrrrrrr2);*/

        Ok(())
    }
    pub async fn create(&self, auth_input: Authentication) -> CtxResult<bool> {
        // ApiResult<Authentication> {
        // let uid = auth_input.clone().id;
        // println!("CREATE AUTH id= {:?}", uid);
        // let query = format!("CREATE {uid} SET timestamp={auth_input.};");
        // let createAuth = self.db.query(query).await?;
        let create_auth: Option<Authentication> =
            self.db.create(TABLE_NAME).content(auth_input).await?;
        // let createAuth: Result<Vec<Authentication>, surrealdb::Error > = self.db.create(TABLE_NAME).content(auth_input).await;
        // dbg!(&createAuth);
        // Ok(!createAuth.is_err())
        Ok(create_auth.is_some())
    }

    pub async fn authenticate(
        &self,
        ctx: &Ctx,
        local_user_id: String,
        auth: AuthType,
    ) -> CtxResult<String> {
        let id = Authentication::get_string_id(
            TABLE_NAME.to_string(),
            local_user_id.clone(),
            auth.clone(),
            true,
        )?;
        let q = "SELECT id FROM <record>$id;".to_string();
        let mut select_authentication = self.db.query(q).bind(("id", id)).await?;
        let rec_found: Option<Thing> = select_authentication.take("id")?;
        match rec_found {
            None => Err(ctx.to_ctx_error(AppError::AuthenticationFail {})),
            Some(_) => Ok(local_user_id),
        }
    }

    pub async fn get_by_auth_type(
        &self,
        local_user_id: String,
        auth_type: AuthType,
    ) -> CtxResult<Vec<Authentication>> {
        // let id = Authentication::to_string_id(TABLE_NAME.parse().unwrap(), local_user_id, auth_type, false)?;

        /*let auth = Authentication{id: id.parse().unwrap(), passkey:None, timestamp:"somett".parse().unwrap() };
                println!("get_by_auth_type id={:?}", auth.id);
                // println!("get_by_auth_type={:?}", auth);
        // let idStr=auth.id;
                let res:Option<Vec<Authentication>> = self.db.select(auth.id).await?;
                dbg!(res);*/

        let a_type = auth_type.as_str();
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
