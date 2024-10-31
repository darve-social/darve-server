use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use futures::stream::Stream as FStream;
use futures::{FutureExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt as _;
use validator::Validate;

use sb_user_auth::entity::access_rule_entity::{AccessRule, AccessRuleDbService};
use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use crate::entity::discussion_entitiy::DiscussionDbService;
use crate::entity::discussion_topic_entitiy::{DiscussionTopic, DiscussionTopicDbService};
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::utils::askama_filter_util::filters;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_user_auth::entity::access_right_entity::AccessRightDbService;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/discussion/:discussion_id/topic", post(create_update))
        .route("/api/discussion/:discussion_id/topic", get(get_form))
        .with_state(state)
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TopicInput {
    pub id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub hidden: Option<String>,
    pub access_rule_id: String,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct DiscussionTopicView {
    pub(crate) id: Thing,
    pub(crate) title: String,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/discussion_topic_items_edit.html")]
pub struct DiscussionTopicItemsEdit {
    pub community_id: Thing,
    pub edit_topic: DiscussionTopicItemForm,
    pub topics: Vec<DiscussionTopic>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/discussion_topic_form.html")]
pub struct DiscussionTopicItemForm {
    pub id: String,
    pub discussion_id: String,
    pub title: String,
    pub hidden: bool,
    pub access_rule: Option<AccessRule>,
    pub access_rules: Vec<AccessRule>,
}

async fn get_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>
) -> CtxResult<DiscussionTopicItemForm> {
    println!("->> {:<12} - create_update_disc_topic", "HANDLER");
    let user_id = LocalUserDbService{db: &_db, ctx: &ctx}.get_ctx_user_thing().await?;

    let disc_id = Thing::try_from(discussion_id).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into discussion Thing".to_string() }))?;
    let disc = DiscussionDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(disc_id.to_raw())).await?;
    let required_diss_auth = Authorization { authorize_record_id: disc_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
    AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&user_id, &required_diss_auth).await?;

    let id:Option<&String> = match qry.get("id").unwrap_or(&String::new()).len()>0 {
        true =>Some(qry.get("id").unwrap()),
        false => None
    };

    let access_rules= AccessRuleDbService{ db: &_db, ctx: &ctx }.get_list(disc.belongs_to).await?;

    let disc_form = match id {
        None => DiscussionTopicItemForm {
            id: String::new(),
            discussion_id: disc_id.clone().to_raw(),
            title: "".to_string(),
            hidden: false,
            access_rule: None,
            access_rules,
        },
        Some(topic_id) => {
            let topic_id = Thing::try_from(topic_id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into topic Thing".to_string() }))?;
            let topic = DiscussionTopicDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(topic_id.to_raw())).await?;
            let access_rule = match topic.access_rule {
                None => None,
                Some(id) => Some(AccessRuleDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(id.to_raw())).await?)
            };
            DiscussionTopicItemForm {
                id: topic.id.unwrap().to_raw(),
                discussion_id: disc_id.to_raw(),
                title: topic.title,
                hidden: topic.hidden,
                access_rule,
                access_rules
            }
        }
    };

    Ok(disc_form)
}

async fn create_update(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       Path(discussion_id): Path<String>,
                       JsonOrFormValidated(form_value): JsonOrFormValidated<TopicInput>,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - create_update_disc_topic", "HANDLER");
    let user_id = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let disc_id = Thing::try_from(discussion_id).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into discussion Thing".to_string() }))?;
    let disc_db_ser = DiscussionDbService { db: &_db, ctx: &ctx };
    let comm_id = disc_db_ser.get(IdentIdName::Id(disc_id.to_raw())).await?.belongs_to;

    let required_diss_auth = Authorization { authorize_record_id: disc_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
    AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&user_id, &required_diss_auth).await?;

    let disc_topic_db_ser = DiscussionTopicDbService { db: &_db, ctx: &ctx };

    let mut update_topic = match form_value.id.len() > 0 {
        false => DiscussionTopic {
            id: None,
            title: "".to_string(),
            access_rule: None,
            hidden: false,
            r_created: None,
        },
        true => {
            Thing::try_from(form_value.id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into topic Thing".to_string() }))?;
            disc_topic_db_ser.get(IdentIdName::Id(form_value.id)).await?
        }
    };

    if form_value.title.len() > 0 {
        update_topic.title = form_value.title;
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "title must have value".to_string() }));
    };
    update_topic.hidden = form_value.hidden.is_some();

    update_topic.access_rule = match form_value.access_rule_id.len()>0 {
        true => Some(Thing::try_from(form_value.access_rule_id).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into access_rule_id Thing".to_string() }))?),
        false => None,
    };

    let res = disc_topic_db_ser
        .create_update(update_topic)
        .await?;
    disc_db_ser.add_topic(disc_id.clone(), res.id.clone().unwrap()).await?;

     let topics = disc_db_ser.get_topics(disc_id.clone()).await?;
    let access_rules = AccessRuleDbService{ db: &_db, ctx: &ctx }.get_list(comm_id.clone()).await?;
    ctx.to_htmx_or_json_res::<DiscussionTopicItemsEdit>( DiscussionTopicItemsEdit {
        community_id: comm_id,
        edit_topic: DiscussionTopicItemForm {
            id: String::new(),
            discussion_id: disc_id.to_raw(),
            title: String::new(),
            hidden: false,
            access_rule: None,
            access_rules,
        },
        topics,
    })
}


#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use sb_middleware::ctx::Ctx;
    use crate::entity::community_entitiy::CommunityDbService;
    use crate::entity::discussion_entitiy::DiscussionDbService;
    use crate::routes::community_routes::{get_community, CommunityInput};
    use crate::routes::discussion_topic_routes::{DiscussionTopicItemsEdit, TopicInput};
    use sb_middleware::utils::db_utils::IdentIdName;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use crate::utils::test_utils::{create_login_test_user, create_test_server};

    #[tokio::test]
    async fn create_discussion() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let comm_name = "community_123";
        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name.clone().to_string(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();

        &create_response.assert_status_success();

        let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
        let comm_db = CommunityDbService { db: &ctx_state._db, ctx: &ctx };
        let comm = comm_db.get(IdentIdName::Id(comm_id.clone().to_raw())).await.expect("community struct");
        let comm_name = comm.name_uri.clone();
        let comm_disc_id = comm.main_discussion.unwrap();

        let disc_db = DiscussionDbService { db: &ctx_state._db, ctx: &ctx };

        // let disc = disc_db.get(IdentIdName::Id(created.id.clone()).into()).await;
        let comm_disc = disc_db.get(IdentIdName::Id(comm_disc_id.to_raw()).into()).await;
        assert_eq!(comm_disc.clone().unwrap().belongs_to.eq(&comm_id.clone()), true);
        // let disc_by_uri = disc_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: disc_name.to_string(), rec: false}).await;
        let discussion = comm_disc.unwrap();
        // let discussion_by_uri = disc_by_uri.unwrap();
        assert_eq!(discussion.clone().topics, None);

        let topic_title = "topic1".to_string();
        let topic_resp = server.post(format!("/api/discussion/{}/topic", comm_disc_id).as_str()).json(&TopicInput{
            id: "".to_string(),
            title: topic_title.clone(),
            hidden: None,
            access_rule_id: "".to_string(),
        }).await;
        &topic_resp.assert_status_success();
        let created = &topic_resp.json::<DiscussionTopicItemsEdit>();
        assert_eq!(&created.community_id, &comm_id);
        assert_eq!(&created.topics.get(0).unwrap().title, &topic_title);
        let topic1_id = created.topics.get(0).unwrap().id.clone();

        let post_name = "post title Name 1".to_string();
        let create_post = server.post(format!("/api/discussion/{comm_disc_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt111").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let comm_view = get_community(State(ctx_state.clone()), ctx.clone(), Path(comm_name.clone()), DiscussionParams{
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 1);

        let post_name = "post title Name 2".to_string();
        let create_post = server.post(format!("/api/discussion/{comm_disc_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt111").add_text("topic_id", topic1_id.clone().unwrap().to_raw() )).await;
            //.json(&PostInput { title: post_name, content: "contentttt".to_string(), topic_id: topic1_id.clone().unwrap().to_raw() }).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let comm_view = get_community(State(ctx_state.clone()), ctx.clone(), Path(comm_name.clone()), DiscussionParams{
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 2);

        let comm_view = get_community(State(ctx_state), ctx, Path(comm_name), DiscussionParams{
            topic_id: topic1_id,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 1);

    }
}

