use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::Deref;
use surrealdb::sql::Thing;
use uuid::Uuid;

use sb_middleware::error::{CtxError, CtxResult, AppError};
use sb_middleware::utils::db_utils::RecordWithId;

const AUTH_DOMAIN_ID_AUTHORIZE_DELIM: &str = "#";
const AUTH_DOMAIN_IDENT_HEIGHT_DELIM: &str = "~";

pub const AUTH_ACTIVITY_VISITOR: &str = "VISITOR";
pub const AUTH_ACTIVITY_MEMBER: &str = "MEMBER";
pub const AUTH_ACTIVITY_EDITOR: &str = "EDITOR";
pub const AUTH_ACTIVITY_ADMIN: &str = "ADMIN";
pub const AUTH_ACTIVITY_OWNER: &str = "OWNER";
const AUTH_ACTIVITY_RANK: [&str; 5] = [AUTH_ACTIVITY_VISITOR, AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_EDITOR, AUTH_ACTIVITY_ADMIN, AUTH_ACTIVITY_OWNER];

// TODO move array to Authentication struct
pub const AUTH_REC_NAME_REPLY: &str = "reply"; //crate::entity::reply_entitiy::TABLE_NAME;
pub const AUTH_REC_NAME_POST: &str ="post"; // crate::entity::post_entitiy::TABLE_NAME;
pub const AUTH_REC_NAME_DISCUSSION: &str = "discussion"; //crate::entity::discussion_entitiy::TABLE_NAME;
pub const AUTH_REC_NAME_COMMUNITY: &str = "community"; //crate::entity::community_entitiy::TABLE_NAME;
pub const AUTH_REC_NAME_SERVER: &str = "server";
pub const AUTH_RECORD_TABLE_RANK: [&str; 5] = [AUTH_REC_NAME_REPLY, AUTH_REC_NAME_POST, AUTH_REC_NAME_DISCUSSION, AUTH_REC_NAME_COMMUNITY, AUTH_REC_NAME_SERVER];

fn get_auth_activity_index(authorize_ident: &str) -> Option<usize> {
    AUTH_ACTIVITY_RANK.iter().position(|&r| r == authorize_ident)
}

pub fn get_auth_record_index(auth_record_name: &String) -> Option<usize> {
    AUTH_RECORD_TABLE_RANK.iter().position(|&r| r == auth_record_name)
}

/*pub fn is_root_auth_rec(auth_rec: Thing) -> bool {
    AUTH_RECORD_TABLE_RANK.last().unwrap().eq(&auth_rec.tb.as_str())
}*/

pub fn has_editor_auth(authorize_ident: &str) -> bool {
    AUTH_ACTIVITY_RANK.iter().position(|a| a.deref().eq(authorize_ident)) >= AUTH_ACTIVITY_RANK.iter().position(|a| a.deref().eq(AUTH_ACTIVITY_EDITOR))
}

pub fn is_any_ge_in_list(compare_to: &Authorization, list: &Vec<Authorization>) -> CtxResult<bool> {
    for a in list {
        match a.ge_equal_ident(compare_to) {
            Ok(is_ge) => {
                if is_ge {
                    return Ok(is_ge);
                }
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    return Ok(false);
}

pub fn get_root_auth_rec_name() -> String {
    AUTH_RECORD_TABLE_RANK.last().unwrap().to_string()
}

pub async fn get_parent_ids(child_rec_id: &Thing, up_to_parent_level_id: Option<&Thing>, ctx: &Ctx, db: &Db) -> CtxResult<Vec<Thing>> {
    let tb1 = if up_to_parent_level_id.is_some() {
        up_to_parent_level_id.unwrap().tb.clone()
    } else { AUTH_REC_NAME_COMMUNITY.to_string() };
    let tb2 = child_rec_id.tb.clone();

    if tb1 == tb2 && tb1 != AUTH_REC_NAME_COMMUNITY {
        return Ok(vec![]);
    }
    let parent_index = get_auth_record_index(&tb1).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb1) }))?;
    let child_index = get_auth_record_index(&tb2).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb2) }))?;

    if parent_index < child_index {
        return Ok(vec![]);
    }
    Ok(get_parent_ids_qry(child_rec_id, ctx, db, parent_index, child_index).await?)
}

async fn get_parent_ids_qry(lower_rec_id: &Thing, ctx: &Ctx, db: &Db, higher_index: usize, lower_index: usize) -> CtxResult<Vec<Thing>> {
    let higher = AUTH_RECORD_TABLE_RANK[lower_index..higher_index + 1].to_vec();

    let mut queries = vec![];
    let mut c = 0;
    while c < higher.len() {
        if c == 0 {
            queries.push(format!("SELECT id FROM {};", lower_rec_id.to_raw()));
        } else {
            // 0 is lower rec so don't include in query
            // let qry = format!("SELECT {} as id FROM {};", higher[1..c + 1].join("."), lower_rec_id.to_raw());
            let qry = format!("SELECT {} as id FROM {};", higher[1..c + 1].iter().map(|_| "belongs_to").collect::<Vec<&str>>().join("."), lower_rec_id.to_raw());
            queries.push(qry);
        }
        c += 1;
    }
    let mut res = db.query(queries.join("")).await?;
    c = 0;
    let mut res_list = vec![];
    while c < queries.len() {
        let res: Option<RecordWithId> = res.take(c)?;
        let res = res.ok_or(ctx.to_api_error(AppError::Generic { description: format!("can not find higher parent record for {}", lower_rec_id.to_raw()) }))?;
        res_list.push(res.id);
        c += 1;
    }

    Ok(res_list)
}

pub async fn is_child_record(parent_rec_id: &Thing, child_rec_id: &Thing, ctx: &Ctx, db: &Db) -> CtxResult<bool> {
    let tb1 = parent_rec_id.tb.clone();
    let tb2 = child_rec_id.tb.clone();
    if tb1 == tb2 {
        return Ok(false);
    }
    let parent_index = get_auth_record_index(&tb1).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb1) }))?;
    let child_index = get_auth_record_index(&tb2).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb2) }))?;

    if parent_index < child_index {
        return Ok(false);
    }
    let same_level_child_id = get_higher_parent_record_id(child_rec_id, ctx, db, parent_index, child_index).await?;
    Ok(parent_rec_id.eq(&same_level_child_id))
}

pub async fn get_same_level_record_ids(rec_id_1: &Thing, rec_id_2: &Thing, ctx: &Ctx, db: &Db) -> CtxResult<(Thing, Thing)> {
    let tb1 = rec_id_1.tb.clone();
    let tb2 = rec_id_2.tb.clone();
    if tb1 == tb2 {
        return Ok((rec_id_1.clone(), rec_id_2.clone()));
    }

    let hierarchy_index1 = get_auth_record_index(&tb1).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb1) }))?;
    let hierarchy_index2 = get_auth_record_index(&tb2).ok_or(ctx.to_api_error(AppError::Generic { description: format!("record tb({}) not found in hierarchy", tb2) }))?;

    if hierarchy_index1 > hierarchy_index2 {
        // get parent for #2
        return Ok((rec_id_1.clone(), get_higher_parent_record_id(rec_id_2, ctx, db, hierarchy_index1, hierarchy_index2).await?));
    } else {
        Ok((get_higher_parent_record_id(rec_id_1, ctx, db, hierarchy_index2, hierarchy_index1).await?, rec_id_2.clone()))
    }
}

async fn get_higher_parent_record_id(lower_rec_id: &Thing, ctx: &Ctx, db: &Db, higher_index: usize, lower_index: usize) -> Result<Thing, CtxError> {
    let higher = AUTH_RECORD_TABLE_RANK[lower_index + 1..higher_index + 1].to_vec();
    let mut q_select_hierarchy = higher.join(".");
    let qry = format!("SELECT {q_select_hierarchy} as id FROM {};", lower_rec_id.to_raw());
    // println!("qqq={qry}");
    let res: Option<RecordWithId> = db.query(qry).await?.take(0)?;

    let res = res.ok_or(ctx.to_api_error(AppError::Generic { description: format!("can not find higher parent record for {}", lower_rec_id.to_raw()) }))?;
    Ok(res.id)
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Authorization {
    // for domain domainId
    pub authorize_record_id: Thing,
    pub authorize_activity: String,
    pub authorize_height: i16,
    // pub social_records_hierarchy: Vec<String>,
}

impl Authorization {
    pub fn new(id: Thing,
               authorize_ident: String,
               authorize_height: i16) -> CtxResult<Self> {
        if get_auth_record_index(&id.tb).is_none() {
            return Err(CtxError { error: AppError::AuthenticationFail, req_id: Uuid::new_v4(), is_htmx: false });
        }
        Ok(Authorization { authorize_record_id: id, authorize_activity: authorize_ident, authorize_height })
    }

    pub async fn ge(&self, compare_to: &Self, ctx: &Ctx, db: &Db) -> CtxResult<bool> {
        let (Authorization { authorize_record_id: id, authorize_activity: authorize_action, authorize_height },
            Authorization { authorize_record_id: id_c, authorize_activity: authorize_action_c, authorize_height: authorize_height_c }) = (self, compare_to);

        // let mut id_c = id_c.clone();

        if get_auth_record_index(&id.tb).is_none() || get_auth_record_index(&id_c.tb).is_none() {
            return Err(CtxError { error: AppError::Generic { description: "Authorization record name must be in range".to_string() }, req_id: Uuid::new_v4(), is_htmx: false });
        }
        if id.ne(&id_c) {
            let is_root_ge = Self::compare_root_ids_ge(id, authorize_action, authorize_height, id_c.clone(), &authorize_action_c, authorize_height_c)?;
            if is_root_ge {
                return Ok(is_root_ge);
            }

            let compare_to_parent_ids = get_parent_ids(&compare_to.authorize_record_id, Some(&self.authorize_record_id), ctx, db).await?;
            let common_parent_record_id = compare_to_parent_ids.into_iter().find(|to_id| to_id.eq(&self.authorize_record_id));
            if common_parent_record_id.is_none() {
                return Ok(false);
            }
            // id_c = common_parent_record_id.unwrap();

        }

        Self::action_or_height_ge(authorize_action, authorize_height, &authorize_action_c, authorize_height_c)
    }

    fn compare_root_ids_ge(id: &Thing, authorize_action: &String, authorize_height: &i16, id_c: Thing, authorize_action_c: &&String, authorize_height_c: &i16) -> CtxResult<bool> {
        let root_rec = get_root_auth_rec_name();
        if id.tb.eq(&root_rec) || id_c.tb.eq(&root_rec) {
            return match (get_auth_record_index(&id.tb), get_auth_record_index(&id_c.tb)) {
                (Some(idx), Some(idxc)) => {
                    if idx != idxc {
                        Ok(idx > idxc)
                    } else {
                        Self::action_or_height_ge(authorize_action, authorize_height, &authorize_action_c, authorize_height_c)
                    }
                }
                (Some(_), None) => Err(CtxError { error: AppError::Generic { description: format!("Can not comapre to non existing AUTH REC (1={} // 2={})", id, id_c) }, req_id: Uuid::new_v4(), is_htmx: false }),
                (None, Some(_)) => Err(CtxError { error: AppError::Generic { description: format!("Can not comapre with non existing AUTH REC (1={} // 2={})", id, id_c) }, req_id: Uuid::new_v4(), is_htmx: false }),
                // should not come here - we check if they are ne()
                (_, _) => Err(CtxError { error: AppError::Generic { description: "Err in authorization neq logic - should not come to this location".to_string() }, req_id: Uuid::new_v4(), is_htmx: false })
            };
        }
        // Err(ApiError { error: Error::Generic { description: format!("Authorization record id not equal- can only compare same record types 1={} /// 2={}", id.to_raw(), id_c.to_raw()) }, req_id: Uuid::new_v4(), is_htmx: false })
        Ok(false)
    }

    pub fn ge_equal_ident(&self, compare_to: &Self) -> CtxResult<bool> {
        let (Authorization { authorize_record_id: id, authorize_activity: authorize_action, authorize_height },
            Authorization { authorize_record_id: id_c, authorize_activity: authorize_action_c, authorize_height: authorize_height_c }) = (self, compare_to);

        if get_auth_record_index(&id.tb).is_none() || get_auth_record_index(&id_c.tb).is_none() {
            return Err(CtxError { error: AppError::Generic { description: "Authorization record name must be in range".to_string() }, req_id: Uuid::new_v4(), is_htmx: false });
        }

        if id.ne(&id_c) && (id.tb == get_root_auth_rec_name() || id_c.tb == get_root_auth_rec_name()) {
            return Self::compare_root_ids_ge(id, authorize_action, authorize_height, id_c.clone(), &authorize_action_c, authorize_height_c);
        }

        if &id != &id_c {
            return Err(CtxError { error: AppError::Generic { description: "This method must compare equal record ids".to_string() }, req_id: Uuid::new_v4(), is_htmx: false });
        }

        Self::action_or_height_ge(authorize_action, authorize_height, &authorize_action_c, authorize_height_c)
    }

    fn action_or_height_ge(authorize_action: &String, authorize_height: &i16, authorize_action_c: &&String, authorize_height_c: &i16) -> CtxResult<bool> {
        return if authorize_action.to_lowercase().ne(&authorize_action_c.to_lowercase()) {
            // idents not equal - we must get result from position in slice or can't rate them
            return match (get_auth_activity_index(authorize_action), get_auth_activity_index(authorize_action_c)) {
                (Some(ai), Some(aic)) => Ok(ai > aic),
                _ => Err(CtxError { error: AppError::Generic { description: "Authorization ident not equal and out of known scale - can't compare.".to_string() }, req_id: Uuid::new_v4(), is_htmx: false })
            };
        } else {
            Ok(authorize_height >= authorize_height_c)
        };
    }
}

#[derive(Debug)]
pub enum AuthorizationError {
    ParseError { reason: String }
}

impl From<Authorization> for String {
    fn from(value: Authorization) -> Self {
        match value {
            Authorization { authorize_record_id: id, authorize_activity: authorize_ident, authorize_height } => {
                let idStr: String = id.to_raw();
                let hStr: String = authorize_height.to_string();
                format!("{idStr}{AUTH_DOMAIN_ID_AUTHORIZE_DELIM}{authorize_ident}{AUTH_DOMAIN_IDENT_HEIGHT_DELIM}{hStr}")
            }
        }
    }
}

impl TryFrom<String> for Authorization {
    type Error = AuthorizationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.split_once(AUTH_DOMAIN_ID_AUTHORIZE_DELIM) {
            Some((domainIdent, auth)) => {
                let domain: Thing = Thing::try_from(domainIdent).map_err(|e| Self::Error::ParseError { reason: "can't parse auth record ident".to_string() })?;
                if get_auth_record_index(&domain.tb).is_none() {
                    return Err(AuthorizationError::ParseError { reason: "wrong domain table ident".to_string() });
                }
                match auth.split_once(AUTH_DOMAIN_IDENT_HEIGHT_DELIM) {
                    Some((authorize_ident, height)) => {
                        let authorize_height = match height.parse::<i16>() {
                            Ok(val) => val,
                            Err(err) => {
                                return Err(AuthorizationError::ParseError { reason: "parse int error i16".to_string() });
                            }
                        };
                        Ok(Authorization { authorize_record_id: domain, authorize_height, authorize_activity: authorize_ident.to_string() })
                    }
                    None => Err(AuthorizationError::ParseError { reason: "can not split on AUTH_DOMAIN_IDENT_HEIGHT_DELIM".to_string() })
                }
            }
            _ => Err(AuthorizationError::ParseError { reason: "can not split on AUTH_DOMAIN_ID_AUTHORIZE_DELIM".to_string() })
        }
    }
}


#[cfg(test)]
mod tests {
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::entity::authorization_entity::{get_root_auth_rec_name, Authorization, AUTH_ACTIVITY_ADMIN, AUTH_ACTIVITY_EDITOR, AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_OWNER, AUTH_ACTIVITY_VISITOR, AUTH_REC_NAME_DISCUSSION, AUTH_REC_NAME_POST};
    use crate::entity::community_entitiy::{Community, CommunityDbService};
    use crate::entity::local_user_entity::LocalUserDbService;
    use crate::routes::community_routes::CommunityInput;
    use sb_community::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::request_utils::CreatedResponse;

    #[tokio::test]
    async fn authorization_compare() {
        let root_rec = "community";

        let a1 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: "general-content".into(), authorize_height: 11 };
        let a2 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: "general-content".into(), authorize_height: 17 };

        let a1Str: String = a1.into();
        let a2Str: String = a2.into();

        let a_1: Authorization = a1Str.try_into().unwrap();
        let a_2: Authorization = a2Str.try_into().unwrap();

        if let Authorization { authorize_record_id: id, authorize_activity, authorize_height } = a_1 {
            assert_eq!(id.tb, root_rec.clone());
            assert_eq!(authorize_activity, "general-content");
            assert_eq!(authorize_height, 11);
        }
        if let Authorization { authorize_record_id: id, authorize_activity, authorize_height } = a_2 {
            assert_eq!(id.tb, root_rec.clone());
            assert_eq!(authorize_activity, "general-content");
            assert_eq!(authorize_height, 17);
        }

        let a1 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_EDITOR.into(), authorize_height: 11 };
        let a2 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_EDITOR.into(), authorize_height: 17 };

        assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

        let a1 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_EDITOR.into(), authorize_height: 17 };
        let a2 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_EDITOR.into(), authorize_height: 17 };

        assert_eq!(a1.ge_equal_ident(&a2).unwrap(), true);

        let a1 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_EDITOR.into(), authorize_height: 11 };
        let a2 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_MEMBER.into(), authorize_height: 17 };

        assert_eq!(a1.ge_equal_ident(&a2).unwrap(), true);

        let a1 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_MEMBER.into(), authorize_height: 112 };
        let a2 = Authorization { authorize_record_id: Thing::from((root_rec.clone(), "123")), authorize_activity: AUTH_ACTIVITY_ADMIN.into(), authorize_height: 17 };

        assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

        let a1 = Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")), authorize_activity: AUTH_ACTIVITY_MEMBER.into(), authorize_height: 112 };
        let a2 = Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")), authorize_activity: AUTH_ACTIVITY_ADMIN.into(), authorize_height: 17 };

        assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

        let a1 = Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")), authorize_activity: AUTH_ACTIVITY_MEMBER.into(), authorize_height: 11 };
        let a2 = Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_DISCUSSION.into(), "123")), authorize_activity: AUTH_ACTIVITY_ADMIN.into(), authorize_height: 17 };
        println!("EXPECTED TEST ERR={:?}", a1.ge_equal_ident(&a2));
        assert_eq!(a1.ge_equal_ident(&a2).is_err(), true);

        let smallerAuth = Authorization { authorize_record_id: Thing::from(("discussion", "234")), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 98 };
        let higherAuth = Authorization { authorize_record_id: Thing::from(("discussion", "234")), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 100 };
        assert_eq!(smallerAuth.ge_equal_ident(&higherAuth).is_err(), false);
        assert_eq!(smallerAuth.clone().ge_equal_ident(&higherAuth).unwrap(), false);


        let compare = Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_DISCUSSION.to_string(), String::from("ggphl6f1ng8v1keac9jl"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 11 };
        let compare_to = [
            Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_DISCUSSION.to_string(), String::from("ggphl6f1ng8v1keac9jl"))), authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(), authorize_height: 99 },
            Authorization { authorize_record_id: Thing::from((AUTH_REC_NAME_DISCUSSION.to_string(), String::from("ggphl6f1ng8v1keac9jl"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 10 },
        ];

        assert_eq!(compare.clone().ge_equal_ident(&compare_to.clone()[0]).unwrap(), true);
        assert_eq!(compare.clone().ge_equal_ident(&compare_to.clone()[1]).unwrap(), true);

        let pos = compare_to.into_iter().position(|a| a.ge_equal_ident(&compare).unwrap());
        assert_eq!(pos, None);

        let compare = Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("qlwk1gd4tjj60gj16dn3"))), authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(), authorize_height: 22 };
        let compare_to = [
            Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("mfoo20kla99blv4mcelp"))), authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(), authorize_height: 99 },
            Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("mfoo20kla99blv4mcelp"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 },
            Authorization { authorize_record_id: Thing::from(("discussion".to_string(), String::from("ggphl6f1ng8v1keac9jl"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 },
        ];

        let pos = compare_to.into_iter().position(|a| compare.ge_equal_ident(&a).unwrap());
        assert_eq!(pos, Some(2));

        let compare = Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("qlwk1gd4tjj60gj16dn3"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 22 };
        let compare_to = [
            Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("mfoo20kla99blv4mcelp"))), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 },
            Authorization { authorize_record_id: Thing::from((get_root_auth_rec_name(), String::from("mfoo20kla99blv4mcelp"))), authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(), authorize_height: 99 },
        ];

        let pos = compare_to.into_iter().position(|a| compare.ge_equal_ident(&a).unwrap());
        assert_eq!(pos, Some(1));
    }

    #[tokio::test]
    async fn get_auth_parent_record_id() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
    }

    #[tokio::test]
    async fn authorize_save() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: "community-123".to_string(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);
        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();
        &create_response.assert_status_success();

        let community_db_service = CommunityDbService { db: &ctx_state._db, ctx: &ctx.clone() };
        let community: Option<Community> = community_db_service.db.select((comm_id.clone().tb, comm_id.id.to_raw())).await.unwrap();

        let comm_discussion_id = community.unwrap().main_discussion.unwrap();

        let user_service = LocalUserDbService { db: &ctx_state._db, ctx: &Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false) };

        // let root_auth_rec = get_root_auth_rec_name();
        let auth_rec = comm_discussion_id;
        let auth_rec2 = comm_id;

        let authorization = Authorization::new(auth_rec.clone(), AUTH_ACTIVITY_OWNER.to_string(), 10).unwrap();
        let authorization_comm_owner = Authorization::new(auth_rec2.clone(), AUTH_ACTIVITY_OWNER.to_string(), 10).unwrap();

        let is_auth = self::is_authorized(&Thing::try_from(user_ident.clone()).unwrap(), &authorization_comm_owner).await;
        assert_eq!(is_auth.is_ok(), true);

        let auth = user_service.authorize(Thing::try_from(user_ident.clone()).unwrap(), authorization, None).await;
        assert_eq!(auth.is_ok(), true);
        let authorization2 = Authorization::new(auth_rec2.clone(), AUTH_ACTIVITY_VISITOR.to_string(), 101);
        dbg!(&authorization2);
        let auth = user_service.authorize(Thing::try_from(user_ident.clone()).unwrap(), authorization2.clone().unwrap(), None).await;
        dbg!(&auth);
        assert_eq!(auth.is_ok(), true);

        let mut authorizations = user_service.get_authorizations(&Thing::try_from(user_ident.clone()).unwrap()).await.unwrap();
        let mut auth_iter = authorizations.iter_mut();
        let is_authorized = user_service.is_authorized(&Thing::try_from(user_ident.clone()).unwrap(), &authorization2.clone().unwrap()).await;
        assert_eq!(auth_iter.len(), 1);
        assert_eq!(is_authorized.is_ok(), true);

        /*let auth_1 = auth_iter.next().unwrap();
        let a_right = AccessRightDbService { db: &ctx_state._db, ctx: &Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false) }
            .get(IdentIdName::ColumnIdent{rec:false, column:"authorization".to_string(), val: auth_1.to}).await.unwrap();
        assert_eq!(a_right.authorization.authorize_activity.clone(), AUTH_ACTIVITY_OWNER.to_string());
        assert_eq!(a_right.authorization.authorize_record_id.tb, auth_rec2.tb);*/

        // TODO test auth on posts etc...
    }

    async fn is_authorized(p0: &Thing, p1: &Authorization) {
        todo!()
    }
}
