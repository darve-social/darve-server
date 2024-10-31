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
    pub(crate) community_view: Option<CommunityView>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/community_view_1.html")]
pub struct CommunityView {
    id: Thing,
    title: String,
    name_uri: String,
    main_discussion: Thing,
    pub(crate) main_discussion_view: Option<DiscussionView>,
}

impl ViewFieldSelector for CommunityView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, title, main_discussion, name_uri".to_string()
    }
}


pub(crate) async fn get_community(State(ctx_state): State<CtxState>,
                                  ctx: Ctx,
                                  Path(name): Path<String>,
                                  q_params: DiscussionParams,
) -> CtxResult<CommunityPage> {
    println!("->> {:<12} - get community", "HANDLER");

    let ident_id_name = match name.contains(":") {
        true => {
            let comm_thing = Thing::try_from(name).map_err(|e| ctx.to_api_error(AppError::Generic { description: "not community Thing".to_string() }))?;
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
        true => Some(Thing::try_from(form_value.id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into community_id Thing".to_string() }))?),
        false => None,
    };

    let mut update_comm = match comm_id {
        None => Community {
            id: match create_custom_id {
                true => Some(Thing::try_from(form_value.id).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into community_id Thing".to_string() }))?),
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
        return Err(ctx.to_api_error(AppError::Generic { description: "title must have value".to_string() }));
    };

    if form_value.name_uri.len() > 0 {
        update_comm.name_uri = form_value.name_uri;
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "name_uri must have value".to_string() }));
    };

    community_db_service
        .create_update(update_comm)
        .await
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use axum_htmx::HX_REQUEST;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::entity::community_entitiy::CommunityDbService;
    use crate::routes::community_routes::CommunityInput;
    use crate::utils::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::db_utils::IdentIdName;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use sb_user_auth::entity::access_right_entity::AccessRightDbService;
    use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};

    #[tokio::test]
    async fn get_community_view() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
        let disc_name = "discName1".to_lowercase();

        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: disc_name.clone(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_uri = created.uri.clone().unwrap();

        &create_response.assert_status_success();

        let comm_db = CommunityDbService { db: &ctx_state._db, ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false) };
        let comm_rec = comm_db
            .get(IdentIdName::Id(comm_id.to_raw())).await;
        assert_eq!(comm_rec.clone().unwrap().main_discussion.is_some(), true);

        let comm_rec = comm_db
            .get(IdentIdName::ColumnIdent { val: comm_uri.clone(), column: "name_uri".to_string(), rec: false }).await;
        assert_eq!(comm_rec.clone().unwrap().main_discussion.is_some(), true);

        let get_response = server.get(format!("/community/{comm_uri}").as_str()).await;
        &get_response.assert_status_success();
        dbg!(get_response);
    }

    #[tokio::test]
    async fn create_community() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
        let comm_name = "discName1".to_lowercase();
        let comm_name2 = "disc_name2".to_lowercase();

        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name.clone(), title: "The Community".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();
        // dbg!(&created);
        let comm_name_created = created.uri.clone().unwrap();
        assert_eq!(comm_name, comm_name_created);
        let comm_id1 = Thing::try_from(created.id.clone()).unwrap();

        &create_response.assert_status_success();

        // same username should return error
        let create_response2 = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name_created.clone(), title: "The Community2".to_string() }).await;
        // dbg!(&create_response2);
        &create_response2.assert_status_bad_request();

        let create_response2 = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name2.clone(), title: "The Community2".to_string() }).await;
        &create_response2.assert_status_success();
        let created_comm2 = create_response2.json::<CreatedResponse>();
        let comm2_id = Thing::try_from(created_comm2.id).unwrap();

        let ctx1 = &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
        let comm_db = CommunityDbService { db: &ctx_state._db, ctx: ctx1 };

        let comm = comm_db.get(IdentIdName::Id(comm_id1.to_raw()).into()).await;
        let comm_by_uri = comm_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: comm_name_created.to_string(), rec: false }).await;
        let community1 = comm.unwrap();
        let community1_by_uri = comm_by_uri.unwrap();
        assert_eq!(community1.clone().name_uri, comm_name_created.clone());
        assert_eq!(community1_by_uri.clone().name_uri, comm_name_created.clone());
        let comm_disc1 = community1.main_discussion.clone().unwrap();

        let comm2 = comm_db.get(IdentIdName::Id(comm2_id.to_raw()).into()).await;
        let comm_by_uri2 = comm_db.get(IdentIdName::ColumnIdent { column: "name_uri".to_string(), val: comm_name2.to_string(), rec: false }).await;
        let community2 = comm2.unwrap();
        let community2_by_uri = comm_by_uri2.unwrap();
        assert_eq!(community2.clone().name_uri, comm_name2.clone());
        assert_eq!(community2_by_uri.clone().name_uri, comm_name2.clone());
        let comm_disc_2 = community2.main_discussion.clone().unwrap();

        let db_service = AccessRightDbService { db: &ctx_state._db, ctx: ctx1 };
        let user_auth = db_service.get_authorizations(&Thing::try_from(user_ident.clone()).unwrap()).await.unwrap();
        let mut user_auth_iter = user_auth.clone().into_iter();

        let user_auth0 = user_auth_iter.next().unwrap();
        assert_eq!(user_auth0.authorize_record_id.eq(&community1.id.unwrap()), true);
        let user_auth1 = user_auth_iter.next().unwrap();
        assert_eq!(user_auth1.authorize_record_id.eq(&comm2_id), true);

        println!("uuu0= {:?} ", user_auth.clone());

        assert_eq!(user_auth.clone().len(), 2);

        let smaller_auth = Authorization { authorize_record_id: community2.clone().id.unwrap(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 98 };
        let higher_auth = Authorization { authorize_record_id: community2.id.unwrap(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 100 };

        assert_eq!(smaller_auth.ge(&higher_auth, ctx1, &ctx_state._db).await.is_err(), false);
        let found: Vec<Authorization> = user_auth.clone().into_iter().filter(|v| v.ge_equal_ident(&smaller_auth).ok().unwrap_or(false)).collect();
        assert_eq!(found.len(), 1);
        let found: Vec<Authorization> = user_auth.into_iter().filter(|v| v.ge_equal_ident(&higher_auth).ok().unwrap_or(false)).collect();
        assert_eq!(found.len(), 0);

        let community_resp = server.get(format!("/community/{comm_name_created}").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

        // dbg!(&community_resp);
        &community_resp.assert_status_success();

        let community_resp = server.get(format!("/community/{comm_name_created}?topic_id=community_topic:345").as_str()).add_header(HX_REQUEST, HeaderValue::from_static("true")).await;

        // dbg!(&community_resp);
        &community_resp.assert_status_success();
    }
}

pub async fn community_admin_access(_db: &Db, ctx: &Ctx, community_id: String) -> CtxResult<(Thing, Community)> {
    let req_by = ctx.user_id()?;
    let user_id = Thing::try_from(req_by).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into user_id Thing".to_string() }))?;

    let comm_id = Thing::try_from(community_id).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into community Thing".to_string() }))?;
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
