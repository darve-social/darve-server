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
use sb_middleware::utils::string_utils::get_string_thing;
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

    let disc_id = get_string_thing(discussion_id)?;
    let disc = DiscussionDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(disc_id.clone())).await?;
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
            let topic_id = get_string_thing(topic_id.clone())?;
            let topic = DiscussionTopicDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(topic_id)).await?;
            let access_rule = match topic.access_rule {
                None => None,
                Some(id) => Some(AccessRuleDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(id)).await?)
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

    let disc_id = get_string_thing(discussion_id)?;
    let disc_db_ser = DiscussionDbService { db: &_db, ctx: &ctx };
    let comm_id = disc_db_ser.get(IdentIdName::Id(disc_id.clone())).await?.belongs_to;

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
            disc_topic_db_ser.get(IdentIdName::Id(get_string_thing(form_value.id)?)).await?
        }
    };

    if form_value.title.len() > 0 {
        update_topic.title = form_value.title;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "title must have value".to_string() }));
    };
    update_topic.hidden = form_value.hidden.is_some();

    update_topic.access_rule = match form_value.access_rule_id.len()>0 {
        true => Some(get_string_thing(form_value.access_rule_id)?),
        false => None,
    };

    let res = disc_topic_db_ser
        .create_update(update_topic)
        .await?;
    disc_db_ser.add_topic(disc_id.clone(), res.id.clone().unwrap()).await?;

     let topics = disc_db_ser.get_topics(disc_id.clone()).await?;
    let access_rules = AccessRuleDbService{ db: &_db, ctx: &ctx }.get_list(comm_id.clone()).await?;
    ctx.to_htmx_or_json::<DiscussionTopicItemsEdit>( DiscussionTopicItemsEdit {
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
