use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use crate::entity::discussion_topic_entitiy::DiscussionTopic;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{exists_entity, get_entity, get_entity_view, get_list_qry, with_not_found_err, IdentIdName, ViewFieldSelector};
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Discussion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    // belongs_to=community
    pub belongs_to: Thing,
    // #[validate(custom(function = "is_some_min_chars"))]
    // pub name_uri: Option<String>,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_room_user_ids: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    pub created_by: Thing,
}

pub struct DiscussionDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "discussion";
pub const DISCUSSION_TOPIC_TABLE_NAME: &str = crate::entity::discussion_topic_entitiy::TABLE_NAME;
pub const USER_TABLE_NAME: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;

impl<'a> DiscussionDbService<'a> {
    pub fn get_table_name() -> & 'static str {
    TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    //DEFINE FIELD name_uri ON TABLE {TABLE_NAME} TYPE option<string> // VALUE string::slug(string::trim($value))
    DEFINE FIELD belongs_to ON TABLE {TABLE_NAME} TYPE record;
    DEFINE FIELD title ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD topics ON TABLE {TABLE_NAME} TYPE option<set<record<{DISCUSSION_TOPIC_TABLE_NAME}>, 25>>;
    DEFINE FIELD chat_room_user_ids ON TABLE {TABLE_NAME} TYPE option<set<record<{USER_TABLE_NAME}>, 125>>;
        // ASSERT record::exists($value);
    DEFINE FIELD created_by ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE_NAME}>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
");
        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Discussion> {
        let opt = get_entity::<Discussion>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, ident_id_name: IdentIdName) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_chatroom_with_users(&self, discussions: Vec<Thing>, user_ids: Vec<Thing>) -> CtxResult<Option<Discussion>> {
        if discussions.len()==0 {
            return Ok(None);
        }
        if  user_ids.len() < 2 {
            return Err(self.ctx.to_api_error(AppError::Generic { description: "Need at least 2 users".to_string() }));
        }

        let qry = format!("SELECT * from {} WHERE chat_room_user_ids CONTAINSALL [{}];",discussions.iter().map(|t|t.to_raw()).collect::<Vec<String>>().join(","), user_ids.iter().map(|t|t.to_raw()).collect::<Vec<String>>().join(","));
        let res = get_list_qry::<Discussion>(self.db, TABLE_NAME.to_string(), qry).await?;
        match res.len() {
            0=>Ok(None),
            1=>Ok(Some(res[0].clone())),
            _=>Err(self.ctx.to_api_error(AppError::Generic {description:format!("Expected 1 result, found {}", res.len())}))
        }
    }

    pub async fn create_update(&self, mut record: Discussion) -> CtxResult<Discussion> {

        let resource = record.id.clone().unwrap_or(Thing::from(( TABLE_NAME.to_string(), Id::ulid() )));

        record.r_created = None;
        let disc: Option<Discussion> = self.db
            .upsert( (resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let disc = disc.unwrap();
        let auth = Authorization { authorize_record_id: disc.id.clone().unwrap(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 };
        let aright_db = AccessRightDbService { db: &self.db, ctx: &self.ctx };
        // TODO in transaction
        aright_db.authorize(disc.created_by.clone(), auth, None).await?;
        Ok(disc)
    }

    pub async fn add_topic(&self, discussion_id: Thing, topic_id: Thing) -> CtxResult<Discussion> {
        let q = "UPDATE $ident SET topics+=$new_topic";

        let disc: Option<Discussion> = self.db
            .query(q)
            .bind(("ident", discussion_id.clone()))
            .bind(("new_topic", topic_id))
            .await?.take(0)?;
        disc.ok_or(self.ctx.to_api_error(AppError::EntityFailIdNotFound { ident: discussion_id.to_raw() }))
    }

    pub async fn get_topics(&self, discussion_id: Thing) -> CtxResult<Vec<DiscussionTopic>> {
        let q = "SELECT topics.*.* FROM $discussion_id;";

        let disc: Option<Vec<DiscussionTopic>> = self.db
            .query(q)
            .bind(("discussion_id", discussion_id.clone()))
            .await?.take("topics")?;
        Ok(disc.unwrap_or(vec![]))
    }
}

