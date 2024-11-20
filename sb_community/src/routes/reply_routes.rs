use std::net::ToSocketAddrs;
use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Router;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;
use sb_middleware::ctx::Ctx;
use crate::entity::discussion_entitiy::DiscussionDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::notification_entitiy::{Notification, NotificationDbService};
use crate::entity::post_entitiy::PostDbService;
use crate::entity::reply_entitiy::{Reply, ReplyDbService};
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;
use crate::routes::community_routes::{CommunityNotificationEvent, PostNotificationEventIdent};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/discussion/:discussion_id/post/:post_uri/reply", post(create_entity))
        .route("/api/discussion/:discussion_id/post/:post_ident/replies", get(get_post_replies))
        .with_state(state)
}

#[derive(Deserialize, Serialize, Validate)]
pub struct PostReplyInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
}

#[derive(Template, Serialize)]
#[template(path = "nera2/post-reply-list-1.html")]
pub struct PostReplyList {
    replies: Vec<PostReplyView>,
}

#[derive(Template, Serialize, Deserialize)]
#[template(path = "nera2/post-reply-1.html")]
pub struct PostReplyView {
    pub id: Thing,
    pub title: String,
    pub content: String,
    pub r_created: String,
}

impl ViewFieldSelector for PostReplyView {
    fn get_select_query_fields(ident: &IdentIdName) -> String {
        "id, title, content, r_created".to_string()
    }
}

async fn get_post_replies(State(CtxState { _db, .. }): State<CtxState>,
                          ctx: Ctx,
                          Path(discussion_id__post_ident): Path<(String, String)>,
) -> CtxResult<PostReplyList> {
    println!("->> {:<12} - get post", "HANDLER");

    let diss_db = DiscussionDbService { db: &_db, ctx: &ctx };
    diss_db.must_exist(IdentIdName::Id(get_string_thing(discussion_id__post_ident.0)?)).await?;

    let ident = Thing::try_from(discussion_id__post_ident.1).map_err(|e| { ctx.to_ctx_error(AppError::Generic { description: "Can not convert to Thing".to_string() }) })?;
    if ident.tb != PostDbService::get_table_name() {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "Post ident wrong".to_string() }));
    }

    let post_replies = ReplyDbService { db: &_db, ctx: &ctx }.get_by_post_desc_view::<PostReplyView>(ident, 0, 120).await?;
    Ok(PostReplyList { replies: post_replies })
}

async fn create_entity(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       Path(discussion_id__post_uri): Path<(String, String)>,
                       JsonOrFormValidated(reply_input): JsonOrFormValidated<PostReplyInput>,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - create_post ", "HANDLER");
    let created_by = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let discussion = Thing::try_from(discussion_id__post_uri.0).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error discussion id into Thing".to_string() }))?;

    let post_db_service = PostDbService { db: &_db, ctx: &ctx };
    let post_id = post_db_service
        .must_exist(IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent { column: "belongs_to".to_string(), val: discussion.to_raw(), rec: true },
            IdentIdName::ColumnIdent { column: "r_title_uri".to_string(), val: discussion_id__post_uri.1, rec: false },
        ])).await?;

    let reply_db_service = ReplyDbService { db: &_db, ctx: &ctx };
    let mut reply = reply_db_service
        .create(Reply {
            id: None,
            discussion,
            belongs_to: post_id.clone(),
            created_by,
            title: reply_input.title,
            content: reply_input.content,
            r_created: None,
            r_updated: None,
        })
        .await?;

    let reply_comm_view = reply_db_service.get_view::<PostReplyView>(&IdentIdName::Id(reply.id.clone().unwrap())).await?;


    let post = post_db_service.increase_replies_nr(post_id.clone()).await?;

    let notif_db_ser = NotificationDbService { db: &_db, ctx: &ctx };

    let event_ident = String::try_from( &PostNotificationEventIdent::from((&reply, &post)) ).ok();
    notif_db_ser.create(
        Notification { id: None, event_ident: event_ident.clone(), event: CommunityNotificationEvent::DiscussionPost_ReplyNrIncreased.to_string(), content: post.replies_nr.to_string(), r_created: None }
    ).await?;

    notif_db_ser.create(
        Notification { id: None, event_ident, event: CommunityNotificationEvent::DiscussionPost_ReplyAdded.to_string(), content: reply_comm_view.render().unwrap(), r_created: None }
    ).await?;

    let res = CreatedResponse { success: true, id: reply.id.unwrap().clone().to_raw(), uri: None };
    ctx.to_htmx_or_json_res::<CreatedResponse>(res)
}
