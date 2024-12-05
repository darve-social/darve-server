use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

use sb_middleware::db;
use sb_middleware::utils::db_utils::{exists_entity, get_entity, with_not_found_err, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct DiscussionTopic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    // #[validate(length(min = 5, message = "Min 5 characters"))]
    // pub discussion: Discussion,
    #[validate(length(min = 5, message = "Min 1 characters"))]
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_rule: Option<Thing>,
    pub hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
}

pub struct DiscussionTopicDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "discussion_topic";

impl<'a> DiscussionTopicDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD title ON TABLE {TABLE_NAME} TYPE string VALUE string::trim($value)
         ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD access_rule ON TABLE {TABLE_NAME} TYPE option<record<access_rule>>;
    DEFINE FIELD hidden ON TABLE {TABLE_NAME} TYPE bool;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
");
        let mutation = self.db.query(sql).await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<DiscussionTopic> {
        let opt =
            get_entity::<DiscussionTopic>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn create_update(&self, mut record: DiscussionTopic) -> CtxResult<DiscussionTopic> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand())));
        record.r_created = None;

        let disc_topic: Option<DiscussionTopic> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        Ok(disc_topic.unwrap())
    }

    /*pub async fn create_titles(&self, titles: Vec<String>) -> ApiResult<Vec<DiscussionTopic>> {
        let records: Vec<DiscussionTopic> = titles.into_iter().map(|title| DiscussionTopic { title: title.clone(), id: None, pricing: None, hidden: false, r_created: None }).collect();
        let mut res: Vec<DiscussionTopic> = vec![];
        for dt in records {
            let savedDt = self.db
                .create(TABLE_NAME)
                .content(dt)
                .await
                .map_err(ApiError::from(self.ctx))?;
            if savedDt.is_some(){
                res.push(savedDt.unwrap());
            }
        }

        Ok(res)
    }*/
}
