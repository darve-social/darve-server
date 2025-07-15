use std::collections::HashMap;
use std::sync::Arc;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use axum_htmx::HX_REDIRECT;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

use crate::database::client::Db;
use crate::entities::community::community_entity;
use crate::entities::user_auth::{access_right_entity, authorization_entity, local_user_entity};
use crate::middleware;
use crate::utils::{askama_filter_util, template_utils};
use access_right_entity::AccessRightDbService;
use askama_filter_util::filters;
use authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};
use community_entity::{Community, CommunityDbService};
use discussion_routes::{get_discussion_view, DiscussionView};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use template_utils::ProfileFormPage;

use super::discussion_routes;

pub fn routes() -> Router<Arc<CtxState>> {
    let view_routes = Router::new()
        .route("/community", get(create_update_form))
        .route("/community/:name", get(get_community));

    Router::new()
        .merge(view_routes)
        .route("/api/community", post(create_update))
}

#[derive(Template, Serialize)]
#[template(path = "nera2/community_form.html")]
struct CommunityForm {
    community_view: Option<CommunityView>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CommunityInput {
    pub id: String,
    // pub create_custom_id: Option<bool>,
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
    title: Option<String>,
    name_uri: String,
    // TODO -rename to default_discussion-
    profile_discussion: Thing,
    pub profile_discussion_view: Option<DiscussionView>,
}

impl ViewFieldSelector for CommunityView {
    fn get_select_query_fields() -> String {
        "id, title, default_discussion as profile_discussion, name_uri".to_string()
    }
}

pub async fn get_community(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(name): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<CommunityPage> {
    let ident_id_name = match name.contains(":") {
        true => {
            let comm_thing = get_string_thing(name)?;
            IdentIdName::Id(comm_thing)
        }
        false => IdentIdName::ColumnIdent {
            column: "name_uri".to_string(),
            val: name.clone(),
            rec: false,
        },
    };
    let mut comm_view = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_view::<CommunityView>(ident_id_name)
    .await?;
    // TODO -rename to profile_discussion_view- change in FE
    comm_view.profile_discussion_view = Some(
        get_discussion_view(
            &ctx_state.db.client,
            &ctx,
            comm_view.profile_discussion.clone(),
            q_params,
        )
        .await?,
    );
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
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Query(qry): Query<HashMap<String, String>>,
) -> CtxResult<ProfileFormPage> {
    Ok(ProfileFormPage::new(
        Box::new(CommunityForm {
            community_view: match qry.get("id") {
                None => None,
                Some(id) => Some(
                    CommunityDbService {
                        db: &state.db.client,
                        ctx: &ctx,
                    }
                    .get_view::<CommunityView>(IdentIdName::Id(get_string_thing(id.clone())?))
                    .await?,
                ),
            },
        }),
        None,
        None,
        None,
    ))
}

async fn create_update(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(form_value): JsonOrFormValidated<CommunityInput>,
) -> CtxResult<Response> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let comm = create_update_community(&state.db.client, &ctx, form_value, &user_id).await?;
    let res = CreatedResponse {
        success: true,
        id: comm.id.unwrap().to_raw(),
        uri: Some(comm.name_uri),
    };
    let uri = res.uri.clone().unwrap();
    let mut res = ctx.to_htmx_or_json::<CreatedResponse>(res)?.into_response();

    res.headers_mut().append(
        HX_REDIRECT,
        format!("/community/{}", uri).as_str().parse().unwrap(),
    );
    Ok(res)
}

pub async fn create_update_community(
    _db: &Db,
    ctx: &Ctx,
    form_value: CommunityInput,
    user_id: &Thing,
) -> CtxResult<Community> {
    let community_db_service = CommunityDbService {
        db: &_db,
        ctx: &ctx,
    };

    let comm_id = match form_value.id.len() > 0 {
        true => Some(get_string_thing(form_value.id.clone())?),
        false => None,
    };

    let mut update_comm = match comm_id {
        None => Community {
            id: None,
            title: None,
            name_uri: "".to_string(),
            default_discussion: None,
            profile_chats: None,
            r_created: None,
            courses: None,
            created_by: user_id.clone(),
            stripe_connect_account_id: None,
            stripe_connect_complete: false,
        },
        Some(comm_id) => {
            // .get throws if not existant community_db_service.must_exist(IdentIdName::Id(comm_id.to_raw())).await?;
            let required_comm_auth = Authorization {
                authorize_record_id: comm_id.clone(),
                authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
                authorize_height: 99,
            };
            AccessRightDbService {
                db: &_db,
                ctx: &ctx,
            }
            .is_authorized(&user_id, &required_comm_auth)
            .await?;
            community_db_service.get(IdentIdName::Id(comm_id)).await?
        }
    };

    if form_value.title.len() > 0 {
        update_comm.title = Some(form_value.title);
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "title must have value".to_string(),
        }));
    };

    if form_value.name_uri.len() > 0 {
        update_comm.name_uri = form_value.name_uri;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "name_uri must have value".to_string(),
        }));
    };

    community_db_service.create_update(update_comm).await
}

pub async fn community_admin_access(
    _db: &Db,
    ctx: &Ctx,
    community_id: String,
) -> CtxResult<(Thing, Community)> {
    let user_id = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let comm_id = get_string_thing(community_id)?;
    let comm = CommunityDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get(IdentIdName::Id(comm_id.clone()))
    .await?;
    let required_comm_auth = Authorization {
        authorize_record_id: comm_id.clone(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 1,
    };
    AccessRightDbService {
        db: &_db,
        ctx: &ctx,
    }
    .is_authorized(&user_id, &required_comm_auth)
    .await?;
    Ok((comm_id, comm))
}

/*#[derive(Serialize, Deserialize, Clone)]
pub struct DiscussionNotificationEventData {
    pub discussion_id: Thing,
    pub topic_id: Option<Thing>,
    pub post_id: Thing,
}

impl From<&DiscussionPostView> for DiscussionNotificationEventData {
    fn from(post: &DiscussionPostView) -> Self {
       DiscussionNotificationEventData {
            discussion_id: post.belongs_to_id.clone(),
            topic_id: post.topic.clone().map(|t| t.id),
            post_id: post.id.clone(),
        }
    }
}

impl From<(&Reply, &Post)> for DiscussionNotificationEventData {
    fn from(data: (&Reply, &Post)) -> Self {
       DiscussionNotificationEventData {
            discussion_id: data.0.discussion.clone(),
            topic_id: data.1.discussion_topic.clone(),
            post_id: data.1.id.clone().unwrap(),
        }
    }
}

impl TryFrom<&DiscussionNotificationEventData> for String {
    type Error = serde_json::Error;

    fn try_from(value: &DiscussionNotificationEventData) -> Result<Self, Self::Error> {
       serde_json::to_string(value)
    }
}

impl TryFrom<String> for DiscussionNotificationEventData {
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(value.as_str())
    }
}

impl TryFrom<Option<String>> for DiscussionNotificationEventData {
    type Error = AppError;

    fn try_from(value: Option<String>) -> Result<Self, Self::Error> {
        match value {
            None => Err(AppError::Generic {description:"TryFrom Option<String> for PostNotificationEventIdent = None".to_string()}),
            Some(val) => DiscussionNotificationEventData::try_from(val).map_err(|e| e.into())
        }
    }
}
*/
