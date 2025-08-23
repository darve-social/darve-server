use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};
use surrealdb::sql::{Id, Thing};

use crate::database::client::Db;
use crate::middleware;
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

use super::{access_right_entity, access_rule_entity, local_user_entity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessGainAction {
    pub id: Option<Thing>,
    pub external_ident: Option<String>,
    pub access_rule_pending: Option<Thing>,
    pub access_rights: Option<Vec<Thing>>,
    pub local_user: Option<Thing>,
    pub action_type: AccessGainActionType,
    pub action_status: AccessGainActionStatus,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,
}

#[derive(EnumString, Display, VariantNames, Debug, Clone, Serialize, Deserialize)]
pub enum AccessGainActionType {
    LocalUser,
    Stripe,
}

#[derive(EnumString, Display, VariantNames, Debug, Clone, Serialize, Deserialize)]
pub enum AccessGainActionStatus {
    Complete,
    Failed,
    Pending,
}

pub struct AccessGainActionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "join_action";
const TABLE_COL_ACCESS_RIGHT: &str = access_right_entity::TABLE_NAME;
const TABLE_COL_ACCESS_RULE: &str = access_rule_entity::TABLE_NAME;
const TABLE_COL_LOCAL_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> AccessGainActionDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS {TABLE_COL_ACCESS_RIGHT}s ON TABLE {TABLE_NAME} TYPE option<set<record<{TABLE_COL_ACCESS_RIGHT}>>>;
    DEFINE FIELD IF NOT EXISTS {TABLE_COL_LOCAL_USER} ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_LOCAL_USER}>>;
    DEFINE INDEX IF NOT EXISTS {TABLE_COL_LOCAL_USER}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_LOCAL_USER};
    DEFINE FIELD IF NOT EXISTS {TABLE_COL_ACCESS_RULE}_pending ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_ACCESS_RULE}>>;
    DEFINE INDEX IF NOT EXISTS {TABLE_COL_ACCESS_RULE}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_ACCESS_RULE}_pending;
    DEFINE FIELD IF NOT EXISTS external_ident ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS action_type ON TABLE {TABLE_NAME} TYPE string ASSERT $value INSIDE {:?};
    DEFINE FIELD IF NOT EXISTS action_status ON TABLE {TABLE_NAME} TYPE string ASSERT $value INSIDE {:?};
    DEFINE INDEX IF NOT EXISTS action_status_idx ON TABLE {TABLE_NAME} COLUMNS action_status;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ", AccessGainActionType::VARIANTS, AccessGainActionStatus::VARIANTS);
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate PaymentAction");

        Ok(())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<AccessGainAction> {
        let opt = get_entity::<AccessGainAction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn create_update(&self, mut record: AccessGainAction) -> CtxResult<AccessGainAction> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));
        record.r_created = None;
        record.r_updated = None;

        let acc_right: Option<AccessGainAction> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(acc_right.unwrap())
    }
    // pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, ident_id_name: &IdentIdName) -> ApiResult<T> {
    //     let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), ident_id_name).await?;
    //     with_not_found_err(opt, self.ctx, ident_id_name.to_string().as_str())
    // }
}
