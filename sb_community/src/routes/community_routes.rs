use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use axum_htmx::HX_REDIRECT;
use futures::stream::Stream as FStream;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use crate::entity::community_entitiy::{Community, CommunityDbService};
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use crate::routes::discussion_routes::{get_discussion_view, DiscussionPostView, DiscussionView};
use sb_user_auth::utils::askama_filter_util::filters;
use sb_user_auth::utils::template_utils::ProfileFormPage;
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use sb_middleware::utils::request_utils::CreatedResponse;
use strum::{Display, EnumString};
use crate::entity::post_entitiy::Post;
use crate::entity::reply_entitiy::Reply;
use crate::routes::discussion_topic_routes::DiscussionTopicView;
use crate::routes::reply_routes::PostReplyView;

pub fn routes(state: CtxState) -> Router {
    let view_routes = Router::new()
        .route("/community", get(create_update_form))
        .route("/community/:name", get(get_community));

    Router::new()
        .merge(view_routes)
        .route("/api/community", post(create_update))
        .with_state(state)
}

#[derive(Template, Serialize)]
#[template(path = "nera2/community_form.html")]
struct CommunityForm {
    community_view: Option<CommunityView>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CommunityInput {
    pub id: String,
    pub create_custom_id: Option<bool>,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub name_uri: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/community_page.html")]
pub struct CommunityPage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    pub community_view: Option<CommunityView>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/community_view_1.html")]
pub struct CommunityView {
    id: Thing,
    title: String,
    name_uri: String,
    main_discussion: Thing,
    pub main_discussion_view: Option<DiscussionView>,
}

impl ViewFieldSelector for CommunityView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, title, main_discussion, name_uri".to_string()
    }
}


pub async fn get_community(State(ctx_state): State<CtxState>,
                           ctx: Ctx,
                           Path(name): Path<String>,
                           q_params: DiscussionParams,
) -> CtxResult<CommunityPage> {
    println!("->> {:<12} - get community", "HANDLER");

    let ident_id_name = match name.contains(":") {
        true => {
            let comm_thing = Thing::try_from(name).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "not community Thing".to_string() }))?;
            IdentIdName::Id(comm_thing.to_raw())
        }
        false => IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: name.clone(), rec: false }
    };
    let mut comm_view = CommunityDbService { db: &ctx_state._db, ctx: &ctx }
        .get_view::<CommunityView>(ident_id_name).await?;
    comm_view.main_discussion_view = Some(get_discussion_view(&ctx_state._db, &ctx, comm_view.main_discussion.to_raw(), q_params).await?);
    Ok(CommunityPage {
        theme_name: "emerald".to_string(),
        window_title: "win win".to_string(),
        nav_top_title: "navtt".to_string(),
        header_title: "headddr".to_string(),
        footer_text: "foooo".to_string(),
        community_view: Some(comm_view),
    })
}

async fn create_update_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<ProfileFormPage> {
    Ok(ProfileFormPage::new(Box::new(
        CommunityForm {
            community_view: match qry.get("id") {
                None => None,
                Some(id) =>
                    Some(CommunityDbService { db: &_db, ctx: &ctx }
                        .get_view::<CommunityView>(IdentIdName::Id(id.clone())).await?)
            },
        }), None, None))
}

async fn create_update(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       JsonOrFormValidated(form_value): JsonOrFormValidated<CommunityInput>,
) -> CtxResult<Response> {
    println!("->> {:<12} - create_update_comm", "HANDLER");
    let user_id = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let comm = create_update_community(&_db, &ctx, form_value, &user_id).await?;
    let res = CreatedResponse { success: true, id: comm.id.unwrap().to_raw(), uri: Some(comm.name_uri) };
    let uri = res.uri.clone().unwrap();
    let mut res = ctx.to_htmx_or_json::<CreatedResponse>(res).into_response();

    res.headers_mut().append(HX_REDIRECT, format!("/community/{}", uri).as_str().parse().unwrap());
    Ok(res)
}

pub async fn create_update_community(_db: &Db, ctx: &Ctx, form_value: CommunityInput, user_id: &Thing) -> CtxResult<Community> {
    let community_db_service = CommunityDbService { db: &_db, ctx: &ctx };

    let create_custom_id = form_value.create_custom_id.unwrap_or(false);
    let comm_id = match form_value.id.len() > 0 && !create_custom_id {
        true => Some(Thing::try_from(form_value.id.clone()).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into community_id Thing".to_string() }))?),
        false => None,
    };

    let mut update_comm = match comm_id {
        None => Community {
            id: match create_custom_id {
                true => Some(Thing::try_from(form_value.id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into community_id Thing".to_string() }))?),
                false => None
            },
            title: "".to_string(),
            name_uri: "".to_string(),
            main_discussion: None,
            profile_chats: None,
            r_created: None,
            courses: None,
            created_by: user_id.clone(),
            stripe_connect_account_id: None,
            stripe_connect_complete: false,
        },
        Some(comm_id) => {
            // .get throws if not existant community_db_service.must_exist(IdentIdName::Id(comm_id.to_raw())).await?;
            let required_comm_auth = Authorization { authorize_record_id: comm_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 };
            AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&user_id, &required_comm_auth).await?;
            community_db_service.get(IdentIdName::Id(comm_id.to_raw())).await?
        }
    };

    if form_value.title.len() > 0 {
        update_comm.title = form_value.title;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "title must have value".to_string() }));
    };

    if form_value.name_uri.len() > 0 {
        update_comm.name_uri = form_value.name_uri;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "name_uri must have value".to_string() }));
    };

    community_db_service
        .create_update(update_comm)
        .await
}

pub async fn community_admin_access(_db: &Db, ctx: &Ctx, community_id: String) -> CtxResult<(Thing, Community)> {
    let req_by = ctx.user_id()?;
    let user_id = Thing::try_from(req_by).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into user_id Thing".to_string() }))?;

    let comm_id = Thing::try_from(community_id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into community Thing".to_string() }))?;
    let comm = CommunityDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(comm_id.clone().to_raw())).await?;
    let required_comm_auth = Authorization { authorize_record_id: comm_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
    AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&user_id, &required_comm_auth).await?;
    Ok((comm_id, comm))
}

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum CommunityNotificationEvent {
    Discussion_PostAdded,
    DiscussionPost_ReplyAdded,
    DiscussionPost_ReplyNrIncreased,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PostNotificationEventIdent {
    pub discussion_id: Thing,
    pub topic_id: Option<Thing>,
    pub post_id: Thing,
}

impl From<&DiscussionPostView> for PostNotificationEventIdent {
    fn from(post: &DiscussionPostView) -> Self {
       PostNotificationEventIdent {
            discussion_id: post.belongs_to_id.clone(),
            topic_id: post.topic.clone().map(|t| t.id),
            post_id: post.id.clone(),
        }
    }
}

impl From<(&Reply, &Post)> for PostNotificationEventIdent {
    fn from(data: (&Reply, &Post)) -> Self {
       PostNotificationEventIdent {
            discussion_id: data.0.discussion.clone(),
            topic_id: data.1.discussion_topic.clone(),
            post_id: data.1.id.clone().unwrap(),
        }
    }
}

impl TryFrom<&PostNotificationEventIdent> for String {
    type Error = serde_json::Error;

    fn try_from(value: &PostNotificationEventIdent) -> Result<Self, Self::Error> {
       serde_json::to_string(value)
    }
}

impl TryFrom<String> for PostNotificationEventIdent{
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(value.as_str())
    }
}

impl TryFrom<Option<String>> for PostNotificationEventIdent{
    type Error = AppError;

    fn try_from(value: Option<String>) -> Result<Self, Self::Error> {
        match value {
            None => Err(AppError::Generic {description:"TryFrom Option<String> for PostNotificationEventIdent = None".to_string()}),
            Some(val) => PostNotificationEventIdent::try_from(val).map_err(|e| e.into())
        }
    }
}
