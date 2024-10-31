use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use surrealdb::sql::Thing;
use webauthn_rs::prelude::{CredentialID, Passkey};

use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};
use sb_middleware::db;
use crate::entity::authorization_entity::Authorization;

/*pub struct PasskeyCredId(String);

impl From<Passkey> for PasskeyCredId {
    fn from(value: Passkey) -> Self {
        PasskeyCredId(value.cred_id())
    }
}*/

#[derive(Clone, Debug, Serialize, Deserialize, EnumString)]
pub enum AuthType {
    PASSWORD(Option<String>), //  password hash
    EMAIL(Option<String>), //  password hash
    PASSKEY(Option<CredentialID>, Option<Passkey>),
    PUBLIC_KEY(Option<String>), // eth account cryptography
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
            AuthType::PUBLIC_KEY(_) => "PUBLIC_KEY"
        }
    }

    pub fn as_val(&self) -> Option<String> {
        match self.clone() {
            AuthType::PASSWORD(pass) => match pass {
                None => None,
                // TODO hash password - https://docs.rs/argon2/0.5.3/argon2/
                Some(pass) => Some(pass.clone())
            },
            AuthType::EMAIL(email) => email,
            AuthType::PASSKEY(passkeyCredId, paksskey) => {
                let mut cid = match passkeyCredId {
                    None => None,
                    Some(passkeyCId) => Some(passkeyCId.clone())
                };
                if cid.is_none() {
                    cid = match paksskey {
                        None => None,
                        Some(pk) => Some(pk.clone().cred_id().clone())
                    }
                }
                match cid {
                    None => None,
                    Some(cid) => Some(STANDARD.encode(cid.to_vec()))
                }
            }
            AuthType::PUBLIC_KEY(pubKey) => pubKey.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authentication {
    pub id: Thing,// AuthenticationId,
    pub local_user: Thing,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey_json: Option<String>,
    pub updated: Option<String>,
}

impl Authentication {
    pub fn new(userId: String, auth: AuthType) -> Result<Authentication, AppError> {
        let passkey_json = match auth.clone() {
            AuthType::PASSKEY(_, pk) => {
                let str_res = serde_json::to_string(&pk);
                if str_res.is_err() {
                    return Err(str_res.map_err(|e| AppError::Serde {source:e.to_string()}).unwrap_err());
                }
                str_res.ok()
            },
            _ => None
        };

        let id = Self::to_string_id(TABLE_NAME.to_string(), userId.clone(), auth.clone(), true)?;
        // println!("IDDDD={}", id);
        Ok(Authentication { id: id.parse().unwrap(), local_user: userId.parse().unwrap(), auth_type: auth.as_str().to_string(), passkey_json, updated: None })
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

    pub fn to_string_id(table: String, local_user_id: String, authType: AuthType, idValRequired: bool) -> CtxResult<String> {
        let hasRequiredParams = match authType.clone() {
            AuthType::PASSWORD(v)
            | AuthType::EMAIL(v)
            | AuthType::PUBLIC_KEY(v) => v.is_some() && idValRequired || v.is_none(),

            AuthType::PASSKEY(pkCId, pk) => ((pkCId.is_some() && idValRequired) || (pk.is_some() && idValRequired)) || !idValRequired,
        };
        if hasRequiredParams == false {
            return Err(CtxError { error: AppError::Generic { description: "Authentication id has no required value".parse().unwrap() }, req_id: Default::default(), is_htmx: false });
        }

        let typeStr = authType.as_str();
        let value = authType.as_val();
        match value {
            None => Ok(format!("{table}:[{local_user_id},'{typeStr}']")),
            Some(val) => Ok(format!("{table}:[{local_user_id},'{typeStr}','{val}']"))
        }/*match value {
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
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD local_user ON TABLE {TABLE_NAME} TYPE record<local_user>;
    DEFINE INDEX local_user_idx ON TABLE {TABLE_NAME} COLUMNS local_user;
    DEFINE FIELD auth_type ON TABLE {TABLE_NAME} TYPE string VALUE string::uppercase($value)
        ASSERT $value INSIDE ['PASSWORD','EMAIL','PUBLIC_KEY','PASSKEY'];
    DEFINE FIELD passkey_json ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD updated ON TABLE {TABLE_NAME} TYPE datetime VALUE time::now();
    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate local user");


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
    pub async fn create(&self, auth_input: Authentication) -> CtxResult<bool> { // ApiResult<Authentication> {
        let uid = auth_input.clone().id;
        println!("CREATE AUTH id= {:?}", uid);
        // let query = format!("CREATE {uid} SET timestamp={auth_input.};");
        // let createAuth = self.db.query(query).await?;
        let createAuth: Option<Authentication> = self.db.create(TABLE_NAME).content(auth_input).await?;
        // let createAuth: Result<Vec<Authentication>, surrealdb::Error > = self.db.create(TABLE_NAME).content(auth_input).await;
        // dbg!(&createAuth);
        // Ok(!createAuth.is_err())
        Ok(createAuth.is_some())
    }

    pub async fn authenticate(&self, ctx: &Ctx, local_user_id: String, auth: AuthType) -> CtxResult<String> {
        let id = Authentication::to_string_id(TABLE_NAME.to_string(), local_user_id.clone(), auth.clone(), true)?;
        println!("authenticate select id={}", id);
        // dbg!(&auth);
        // TODO replace with query value params - bind()
        let q = format!("SELECT id FROM {id};");
        let mut selectAuthentication = self.db.query(q).await?;
        let recFound: Option<Thing> = selectAuthentication.take("id")?;
        // dbg!(&selectAuthentication.check());
        match recFound {
            None => Err(ctx.to_api_error(AppError::AuthenticationFail { })),
            Some(_) => Ok(local_user_id)
        }
    }

    pub async fn get_by_auth_type(&self, local_user_id: String, auth_type: AuthType) -> CtxResult<Vec<Authentication>> {
        // let id = Authentication::to_string_id(TABLE_NAME.parse().unwrap(), local_user_id, auth_type, false)?;

        /*let auth = Authentication{id: id.parse().unwrap(), passkey:None, timestamp:"somett".parse().unwrap() };
        println!("get_by_auth_type id={:?}", auth.id);
        // println!("get_by_auth_type={:?}", auth);
// let idStr=auth.id;
        let res:Option<Vec<Authentication>> = self.db.select(auth.id).await?;
        dbg!(res);*/

        let a_type = auth_type.as_str();
        let q = format!("SELECT * FROM {TABLE_NAME} WHERE local_user={local_user_id} AND auth_type='{a_type}';");
        let mut res = self.db.query(q).await;
        if res.is_err() {
            dbg!(res);
            return Ok(vec![]);
        }

        let mut response = res.unwrap();
        dbg!(&response);
        let rec = response.take(0)?;

        /*if let Some(authRec) = rec.clone() {
            records.push(authRec);
        }
    }*/
        dbg!(&rec);
        Ok(rec)
    }
}
