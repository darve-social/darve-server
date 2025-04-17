use serde::{Deserialize, Serialize};
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use validator::Validate;

use crate::entity::discussion_entitiy::Discussion;
use sb_middleware::db;
use sb_middleware::error::AppResult;
use sb_middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Community {
    // random id or local_user_id for profile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub name_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_discussion: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_chats: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courses: Option<Vec<Thing>>,
    pub created_by: Thing,
    pub stripe_connect_account_id: Option<String>,
    pub stripe_connect_complete: bool,
}

pub struct CommunityDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "community";
pub const DISCUSSION_TABLE_NAME: &str = crate::entity::discussion_entitiy::TABLE_NAME;

impl<'a> CommunityDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD title ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD name_uri ON TABLE {TABLE_NAME} TYPE string VALUE string::slug(string::trim($value)) OR string::slug(string::trim($this.title))
         ASSERT string::len(string::slug(string::trim($value)))>4 OR string::len(string::slug(string::trim($this.title)))>4;
    DEFINE INDEX community_name_uri_idx ON TABLE {TABLE_NAME} COLUMNS name_uri UNIQUE;
    DEFINE FIELD profile_discussion ON TABLE {TABLE_NAME} TYPE option<record<{DISCUSSION_TABLE_NAME}>>;
    // DEFINE FIELD following_posts_discussion ON TABLE {TABLE_NAME} TYPE option<record<{DISCUSSION_TABLE_NAME}>>;
    DEFINE FIELD courses ON TABLE {TABLE_NAME} TYPE option<array<record<course>>>;
    DEFINE FIELD created_by ON TABLE {TABLE_NAME} TYPE record<local_user>;
    DEFINE FIELD profile_chats ON TABLE {TABLE_NAME} TYPE option<set<record<{DISCUSSION_TABLE_NAME}>>>;
    DEFINE FIELD stripe_connect_account_id ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD stripe_connect_complete ON TABLE {TABLE_NAME} TYPE bool DEFAULT false;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Community> {
        let opt = get_entity::<Community>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn create_update(&self, mut record: Community) -> CtxResult<Community> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::ulid())));
        record.r_created = None;
        // TODO in transaction

        let comm: Option<Community> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let mut comm = comm.unwrap();

        let comm_id = comm.id.clone().unwrap();
        if comm.profile_discussion.is_none() {
            let disc = self
                .db
                .create(DISCUSSION_TABLE_NAME)
                .content(Discussion {
                    id: None,
                    title: None,
                    image_uri: None,
                    topics: None,
                    chat_room_user_ids: None,
                    latest_post_id: None,
                    r_created: None,
                    belongs_to: comm_id.clone(),
                    created_by: comm.created_by.clone(),
                })
                .await
                .map_err(CtxError::from(self.ctx))
                .map(|v: Option<Discussion>| v.unwrap())?;

            let comm_upd: CtxResult<Option<Community>> = self
                .db
                .update((&comm_id.tb, comm_id.id.clone().to_raw()))
                .patch(PatchOp::replace("/profile_discussion", disc.id.clone()))
                .await
                .map_err(CtxError::from(self.ctx));
            comm = comm_upd?.ok_or(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: comm_id.to_raw(),
            }))?;
        }

        let auth1 = Authorization {
            authorize_record_id: comm_id,
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        };
        let aright_db = AccessRightDbService {
            db: &self.db,
            ctx: &self.ctx,
        };
        aright_db
            .authorize(comm.created_by.clone(), auth1, None)
            .await?;

        Ok(comm)
    }

    pub async fn add_profile_chat_discussion(
        &self,
        user_id: Thing,
        discussion_id: Thing,
    ) -> AppResult<()> {
        // !!! needs profile community already
        // or like discussion_entity.add_topic
        let user_comm = self.get_profile_community(user_id).await?;

        let p_chats = user_comm.profile_chats.unwrap_or(vec![]);
        if !p_chats.contains(&discussion_id) {
            let comm_id = user_comm.id.clone().unwrap();
            let _res: Option<Community> = self
                .db
                .update((comm_id.tb, comm_id.id.to_string()))
                .patch(PatchOp::add(
                    "/profile_chats".to_string().as_str(),
                    [discussion_id],
                ))
                .await?;
        }
        Ok(())
    }

    pub async fn get_profile_community(&self, user_id: Thing) -> CtxResult<Community> {
        let user_comm_id = Self::get_profile_community_id(user_id.clone());
        let user_comm = self.get(IdentIdName::Id(user_comm_id.clone())).await;
        match user_comm {
            Ok(_) => user_comm,
            Err(err) => {
                if let AppError::EntityFailIdNotFound { .. } = err.error {
                    self.create_update(Community {
                        id: Some(user_comm_id),
                        title: None,
                        name_uri: user_id.to_raw(),
                        profile_discussion: None,
                        profile_chats: None,
                        r_created: None,
                        courses: None,
                        created_by: user_id.clone(),
                        stripe_connect_account_id: None,
                        stripe_connect_complete: false,
                    })
                    .await
                } else {
                    Err(self.ctx.to_ctx_error(err.error))
                }
            }
        }
    }

    pub fn get_profile_community_id(user_id: Thing) -> Thing {
        Thing::from((Self::get_table_name().to_string(), user_id.id.to_raw()))
    }
}
