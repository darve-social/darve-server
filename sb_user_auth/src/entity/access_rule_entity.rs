use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

use crate::entity::authorization_entity::Authorization;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_list, get_entity_view, with_not_found_err, IdentIdName,
    ViewFieldSelector,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct AccessRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub target_entity_id: Thing,
    #[validate(length(min = 5, message = "Min 1 characters"))]
    pub title: String,
    // can use low authorize_height values for subscriptions (for ex. 0-1000000) so plans can be compared with .ge() and exact authorize_height (for ex. 1000000+) for product id comparison
    // can add functionality so .ge() comparison is below 1000000 and if required authorize_height is 1000000+ then exact check is made
    pub authorization_required: Authorization,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_amount: Option<i32>,
    // how long delivery is possible - how long subsctiption lasts
    pub available_period_days: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_gain_action_confirmation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_gain_action_redirect_url: Option<String>,
    // how many times can it be delivered - 1 if it's a physical product
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub available_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
}

pub struct AccessRuleDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "access_rule";

impl<'a> AccessRuleDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS target_entity_id ON TABLE {TABLE_NAME} TYPE record;
    DEFINE INDEX IF NOT EXISTS target_entity_id_idx ON TABLE {TABLE_NAME} COLUMNS target_entity_id;
    DEFINE FIELD IF NOT EXISTS title ON TABLE {TABLE_NAME} TYPE string VALUE string::trim($value)
         ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS authorization_required ON TABLE {TABLE_NAME} FLEXIBLE TYPE {{ authorize_record_id: record, authorize_activity: string, authorize_height: int}};
    DEFINE FIELD IF NOT EXISTS price_amount ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS available_period_days ON TABLE {TABLE_NAME} TYPE option<number>;
    // DEFINE FIELD IF NOT EXISTS available_amount ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS access_gain_action_confirmation ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS access_gain_action_redirect_url ON TABLE {TABLE_NAME} TYPE option<string>;
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<AccessRule> {
        let opt = get_entity::<AccessRule>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_list(&self, target_entity_id: Thing) -> CtxResult<Vec<AccessRule>> {
        get_entity_list::<AccessRule>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "target_entity_id".to_string(),
                val: target_entity_id.to_raw(),
                rec: true,
            },
            None,
        )
        .await
    }

    pub async fn create_update(&self, mut record: AccessRule) -> CtxResult<AccessRule> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));
        record.r_created = None;

        let disc_topic: Option<AccessRule> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(disc_topic.unwrap())
    }
}
