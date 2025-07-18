use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::sql::Thing;
use validator::Validate;

use access_right_entity::AccessRightDbService;
use access_rule_entity::{AccessRule, AccessRuleDbService};
use authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use discussion_entity::DiscussionDbService;
use discussion_topic_entity::{DiscussionTopic, DiscussionTopicDbService};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::utils::string_utils::get_string_thing;
use utils::askama_filter_util::filters;

use crate::entities::community::{discussion_entity, discussion_topic_entity};
use crate::entities::user_auth::{
    access_right_entity, access_rule_entity, authorization_entity, local_user_entity,
};
use crate::{middleware, utils};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/discussion/:discussion_id/topic", post(create_update))
        .route("/api/discussion/:discussion_id/topic", get(get_form))
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
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Query(qry): Query<HashMap<String, String>>,
) -> CtxResult<DiscussionTopicItemForm> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let disc_id = get_string_thing(discussion_id)?;
    let disc = DiscussionDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get(IdentIdName::Id(disc_id.clone()))
    .await?;
    let required_diss_auth = Authorization {
        authorize_record_id: disc_id.clone(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 1,
    };
    AccessRightDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .is_authorized(&user_id, &required_diss_auth)
    .await?;

    let id: Option<&String> = match qry.get("id").unwrap_or(&String::new()).len() > 0 {
        true => Some(qry.get("id").unwrap()),
        false => None,
    };

    let access_rules = AccessRuleDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_list(disc.belongs_to)
    .await?;

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
            let topic = DiscussionTopicDbService {
                db: &state.db.client,
                ctx: &ctx,
            }
            .get(IdentIdName::Id(topic_id))
            .await?;
            let access_rule = match topic.access_rule {
                None => None,
                Some(id) => Some(
                    AccessRuleDbService {
                        db: &state.db.client,
                        ctx: &ctx,
                    }
                    .get(IdentIdName::Id(id))
                    .await?,
                ),
            };
            DiscussionTopicItemForm {
                id: topic.id.unwrap().to_raw(),
                discussion_id: disc_id.to_raw(),
                title: topic.title,
                hidden: topic.hidden,
                access_rule,
                access_rules,
            }
        }
    };

    Ok(disc_form)
}

async fn create_update(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    JsonOrFormValidated(form_value): JsonOrFormValidated<TopicInput>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let disc_id = get_string_thing(discussion_id)?;
    let disc_db_ser = DiscussionDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let comm_id = disc_db_ser
        .get(IdentIdName::Id(disc_id.clone()))
        .await?
        .belongs_to;

    let required_diss_auth = Authorization {
        authorize_record_id: disc_id.clone(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 1,
    };
    AccessRightDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .is_authorized(&user_id, &required_diss_auth)
    .await?;

    let disc_topic_db_ser = DiscussionTopicDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let mut update_topic = match form_value.id.len() > 0 {
        false => DiscussionTopic {
            id: None,
            title: "".to_string(),
            access_rule: None,
            hidden: false,
            r_created: None,
        },
        true => {
            disc_topic_db_ser
                .get(IdentIdName::Id(get_string_thing(form_value.id)?))
                .await?
        }
    };

    if form_value.title.len() > 0 {
        update_topic.title = form_value.title;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "title must have value".to_string(),
        }));
    };
    update_topic.hidden = form_value.hidden.is_some();

    update_topic.access_rule = match form_value.access_rule_id.len() > 0 {
        true => Some(get_string_thing(form_value.access_rule_id)?),
        false => None,
    };

    let res = disc_topic_db_ser.create_update(update_topic).await?;
    disc_db_ser
        .add_topic(disc_id.clone(), res.id.clone().unwrap())
        .await?;

    let topics = disc_db_ser.get_topics(disc_id.clone()).await?;
    let access_rules = AccessRuleDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_list(comm_id.clone())
    .await?;
    ctx.to_htmx_or_json::<DiscussionTopicItemsEdit>(DiscussionTopicItemsEdit {
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
