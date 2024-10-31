use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use std::ops::{Add, Deref};
use std::time::Duration;
use surrealdb::sql::{Datetime as DatetimeSur, Id, Thing};
use validator::Validate;


use crate::entity::access_rule_entity::AccessRuleDbService;
use crate::entity::authorization_entity::{get_parent_ids, Authorization, AUTH_ACTIVITY_OWNER};
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity_list, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct AccessRight {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub local_user: Thing,
    pub access_rule: Option<Thing>,
    pub authorization: Authorization,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_actions: Option<Vec<Thing>>,
    pub join_action: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_chrono_as_sql_datetime")]
    pub expires_at: Option<DateTime<Utc>>,
    // number of times left to serve - usually None - can count for limited access products
    // pub available_left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<DateTime<Utc>>,
    pub r_updated: Option<DateTime<Utc>>,
}
fn serialize_chrono_as_sql_datetime<S>(x: &Option<chrono::DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    match x {
        None => Some(DatetimeSur::default()).serialize(s),
        Some(x) => Some(Into::<DatetimeSur>::into(*x)).serialize(s)
    }
}

pub struct AccessRightDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "access_right";
const TABLE_NAME_ACCESS_RULE: &str = crate::entity::access_rule_entity::TABLE_NAME;
const TABLE_NAME_LOCAL_USER: &str = crate::entity::local_user_entity::TABLE_NAME;
const TABLE_NAME_PAYMENT_ACTION: &str = crate::entity::payment_action_entitiy::TABLE_NAME;

impl<'a> AccessRightDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD {TABLE_NAME_LOCAL_USER} ON TABLE {TABLE_NAME} type record<{TABLE_NAME_LOCAL_USER}>;
    DEFINE INDEX {TABLE_NAME_LOCAL_USER}_idx on TABLE {TABLE_NAME} columns {TABLE_NAME_LOCAL_USER};
    DEFINE FIELD {TABLE_NAME_ACCESS_RULE} ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_NAME_ACCESS_RULE}>>;
    DEFINE FIELD authorization ON TABLE {TABLE_NAME} FLEXIBLE TYPE {{ authorize_record_id: record, authorize_activity: string, authorize_height: int}};
    DEFINE FIELD {TABLE_NAME_PAYMENT_ACTION}s ON TABLE {TABLE_NAME} TYPE option<set<record<{TABLE_NAME_PAYMENT_ACTION}>>>;
    DEFINE FIELD expires_at ON TABLE {TABLE_NAME} TYPE option<datetime>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
");
        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }

   /* pub async fn must_exist(&self, ident: IdentIdName) -> ApiResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }*/

    /*pub async fn get(&self, ident_id_name: IdentIdName) -> ApiResult<AccessRight> {
        let opt = get_entity::<AccessRight>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }*/

    pub async fn list_by_user(&self, user_id: &Thing) -> CtxResult<Vec<AccessRight>>{
        get_entity_list::<AccessRight>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdent {column:TABLE_NAME_LOCAL_USER.to_string(), val:user_id.to_raw(), rec:true}, None).await
    }

    pub async fn create_update(&self, mut record: AccessRight) -> CtxResult<AccessRight> {
        let resource = record.id.clone().unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::ulid() )));
        record.r_created = None;
        record.r_updated = None;

        let acc_right: Option<AccessRight> = self.db
            .upsert( (resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(acc_right.unwrap())
    }

    pub(crate) async fn has_access_right_ge(&self, user_id: &Thing, authorization: &Authorization) -> CtxResult<bool> {
        // get hierarchy for authorization record and check every hierarchy item with all user auth records for matches - if any ge return true
        //
        let auth_list = self.get_authorizations(user_id).await?;
        if auth_list.len() < 1 {
            return return Ok(false);
        }

        let compare_to_parents_ids = get_parent_ids(&authorization.authorize_record_id, None, self.ctx, self.db).await?;
        for compare_parent_id in compare_to_parents_ids {
            if let Some(_user_auth_ge) = auth_list.iter().find(|a| {
                // find user auth with same parent record id and ge auth values
                a.deref().authorize_record_id.eq(&compare_parent_id)
                    && a.ge_equal_ident(&Authorization { authorize_record_id: compare_parent_id.clone(), authorize_activity: authorization.authorize_activity.clone(), authorize_height: authorization.authorize_height }).unwrap_or(false)
            }) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn authorize(&self, user_id: Thing, authorization: Authorization, expires_at: Option<DateTime<Utc>>) -> CtxResult<()> {
        let has_ge =self.has_access_right_ge(&user_id, &authorization).await?;
        if has_ge {
            return Ok(());
        }

        self.create_update(AccessRight {
            id: None,
            local_user: user_id.clone(),
            access_rule: None,
            authorization,
            payment_actions: None,
            join_action: None,
            expires_at,
            r_created: None,
            r_updated: None,
        }).await?;

        Ok(())
    }

    pub async fn get_authorizations(&self, user_id: &Thing) -> CtxResult<Vec<Authorization>> {
        let now = Utc::now();
        let access_rights = self.list_by_user(user_id).await?;
        let authorizations = access_rights.iter().map(|a_right| {
            if a_right.expires_at.unwrap_or(now.add(Duration::from_secs(1))) > now {
                Some(a_right.authorization.clone())
            } else { None }
        }).filter(|opt_auth| opt_auth.is_some())
            .map(|opt| opt.unwrap())
            .collect();
        Ok(authorizations)
    }

    // TODO set AUTH_RECORD_TABLE_RANK hierarchy array in app's ctx_state and send the hierarchy as parameter here
    pub async fn is_authorized(&self, user_id: &Thing, authorization: &Authorization) -> CtxResult<()> {
        let res = self.has_access_right_ge(user_id, authorization).await?;
        if !res {
            return Err(self.ctx.to_api_error(AppError::AuthorizationFail { required: authorization.clone().into() }));
        }
        Ok(())
    }


    pub async fn add_paid_access_right(&self, local_user_id: Thing, access_rule_id: Thing, payment_action_id: Thing) -> CtxResult<AccessRight> {
        /*let p_action = PaymentActionDbService { ctx: self.ctx, db: self.db }.get(IdentIdName::Id(payment_action.to_raw())).await?;
        if !p_action.paid {
            return Err(self.ctx.to_api_error(Error::Generic { description: "Can not add unpaid access right".to_string() }));
        }*/
        // TODO in transaction
        let paid_access_rule = AccessRuleDbService { db: self.db, ctx: self.ctx }.get(IdentIdName::Id(access_rule_id.to_raw())).await?;

        let access_rights = self.list_by_user(&local_user_id).await?;
        let existing_access_right = match access_rights.len()>0 {
            true => access_rights.into_iter().find(|a_right| a_right.authorization.eq(&paid_access_rule.authorization_required)),
            false => None
        };

        let save_access_right = match existing_access_right {
            None => {
                let expires_at: Option<DateTime<Utc>> = match paid_access_rule.available_period_days {
                    None => None,
                    Some(lasts_days) => Some(Utc::now().add(Duration::from_secs(lasts_days * 60 * 60 * 24)))
                };
                AccessRight {
                    id: None,
                    local_user: local_user_id,
                    access_rule: Some(access_rule_id),
                    authorization: paid_access_rule.authorization_required,
                    payment_actions: Some(vec![payment_action_id]),
                    join_action: None,
                    expires_at,
                    r_created: None,
                    r_updated: None,
                }
            }
            Some(mut a_right) => {
                let current_extended_to = a_right.expires_at.unwrap_or(Utc::now());
                let next_expires_date = match paid_access_rule.available_period_days {
                    None => None,
                    Some(extend_for_days) => Option::from(current_extended_to.add(Duration::from_secs(extend_for_days * 60 * 60 * 24)))
                };
                a_right.expires_at = next_expires_date;
                a_right.payment_actions = if a_right.payment_actions.is_some() {
                    let mut existing_p_actions = a_right.payment_actions.clone().unwrap();
                    existing_p_actions.push(payment_action_id);
                    Option::from(existing_p_actions)
                } else { Some(vec![payment_action_id]) };
                a_right
            }
        };

        let a_right = self.create_update(save_access_right).await?;
        Ok(a_right)
    }


    pub async fn has_owner_access(&self, target_record_id: String) -> CtxResult<Thing> {
        let req_by = self.ctx.user_id()?;
        let user_id = Thing::try_from(req_by).map_err(|e| self.ctx.to_api_error(AppError::Generic { description: "error into user_id Thing".to_string() }))?;

        let target_rec_thing = Thing::try_from(target_record_id).map_err(|e| self.ctx.to_api_error(AppError::Generic { description: "error into community Thing".to_string() }))?;
        let required_auth = Authorization { authorize_record_id: target_rec_thing.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
        self.is_authorized(&user_id, &required_auth).await?;
        Ok(target_rec_thing)
    }

}
