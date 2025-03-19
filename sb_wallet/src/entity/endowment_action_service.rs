use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};
use surrealdb::sql::{Id, Thing};

use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndowmentAction {
    pub id: Option<Thing>,
    pub external_ident: Option<String>,
    pub access_rule_pending: Option<Thing>,
    pub access_rights: Option<Vec<Thing>>,
    pub local_user: Option<Thing>,
    pub action_type: EndowmentActionType,
    pub action_status: EndowmentActionStatus,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,
}

#[derive(EnumString, Display, VariantNames, Debug, Clone, Serialize, Deserialize)]
pub enum EndowmentActionType {
    LocalUser,
    Stripe,
}

#[derive(EnumString, Display, VariantNames, Debug, Clone, Serialize, Deserialize)]
pub enum EndowmentActionStatus {
    Complete,
    Failed,
    Pending,
}

pub struct EndowmentActionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "join_action";
const TABLE_COL_ACCESS_RIGHT: &str = "endowment_access_right";
const TABLE_COL_ACCESS_RULE: &str = "endowment_access_rule";
const TABLE_COL_LOCAL_USER: &str = "";

impl<'a> EndowmentActionDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD {TABLE_COL_ACCESS_RIGHT}s ON TABLE {TABLE_NAME} TYPE option<set<record<{TABLE_COL_ACCESS_RIGHT}>>>;
    DEFINE FIELD {TABLE_COL_LOCAL_USER} ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_LOCAL_USER}>>;
    DEFINE INDEX {TABLE_COL_LOCAL_USER}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_LOCAL_USER};
    DEFINE FIELD {TABLE_COL_ACCESS_RULE}_pending ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_ACCESS_RULE}>>;
    DEFINE INDEX {TABLE_COL_ACCESS_RULE}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_ACCESS_RULE}_pending;
    DEFINE FIELD external_ident ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD action_type ON TABLE {TABLE_NAME} TYPE string ASSERT $value INSIDE {:?};
    DEFINE FIELD action_status ON TABLE {TABLE_NAME} TYPE string ASSERT $value INSIDE {:?};
    DEFINE INDEX action_status_idx ON TABLE {TABLE_NAME} COLUMNS action_status;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ", EndowmentActionType::VARIANTS, EndowmentActionStatus::VARIANTS);
        let mutation = self.db.query(sql).await?;
        &mutation.check().expect("should mutate PaymentAction");

        Ok(())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<EndowmentAction> {
        let opt = get_entity::<EndowmentAction>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn create_update(&self, mut record: EndowmentAction) -> CtxResult<EndowmentAction> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));
        record.r_created = None;
        record.r_updated = None;

        let rec_id = record.id.clone();
        let acc_right: Option<EndowmentAction> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(acc_right.unwrap())
    }
}
