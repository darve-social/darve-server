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
use validator::Validate;

use crate::entity::access_right_entity::AccessRightDbService;
use crate::entity::access_rule_entity::{AccessRule, AccessRuleDbService};
use crate::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_OWNER};
use crate::entity::local_user_entity::LocalUserDbService;
use crate::utils::askama_filter_util::filters;
use crate::utils::template_utils::ProfileFormPage;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{record_exists, IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/community/:community_id/access-rule", get(get_form_page))
        .route("/api/community/:target_record_id/access-rule", get(get_form))
        .route("/api/access-rule", post(create_update))
        .with_state(state)
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/access_rule_form.html")]
pub struct AccessRuleForm {
    pub access_rule: AccessRule,
    pub access_rules: Vec<AccessRule>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct AccessRuleInput {
    pub id: String,
    #[validate(length(min = 3, message = "Min 3 characters"))]
    pub target_entity_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub authorize_record_id_required: String,
    pub authorize_activity_required: String,
    pub authorize_height_required: i16,
    pub price_amount: String,
    pub available_period_days: String,
    pub join_confirmation: String,
    pub join_redirect_url: String,
}

async fn get_form_page(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(community_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<ProfileFormPage> {
    println!("->> {:<12} - get_form access rule", "HANDLER");
    let form = get_form(State(ctx_state), ctx, Path(community_id), Query(qry)).await?;
    Ok(ProfileFormPage::new(Box::new(form), None, None))
}

async fn get_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(target_record_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<AccessRuleForm> {
    println!("->> {:<12} - get_form access rule", "HANDLER");

    let target_id = AccessRightDbService{ db: &_db, ctx: &ctx }.has_owner_access(target_record_id).await?;

    let id_str = qry.get("id").unwrap_or(&"".to_string()).to_owned();
    let id: Option<String> = match id_str.len() > 0 {
        true => Some(id_str),
        false => None
    };

    let access_rule = match id {
        None => AccessRule {
            id: None,
            target_entity_id: target_id.clone(),
            title: String::new(),
            authorization_required: Authorization {
                authorize_record_id: target_id.clone(),
                authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(),
                authorize_height: 0,
            },
            available_period_days: None,
            join_confirmation: None,
            join_redirect_url: None,
            r_created: None,
            price_amount: None,
        },
        Some(id) => {
            AccessRuleDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(id)).await?
        }
    };

    let access_rules = AccessRuleDbService { db: &_db, ctx: &ctx }.get_list(target_id).await?;
    Ok(AccessRuleForm { access_rule, access_rules })
}

async fn create_update(State(ctx_state): State<CtxState>,
                       ctx: Ctx,
                       JsonOrFormValidated(form_value): JsonOrFormValidated<AccessRuleInput>,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - create_update_arule", "HANDLER");
    let user_id = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user_thing().await?;

    let comm_id = Thing::try_from(form_value.target_entity_id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into community Thing".to_string() }))?;
    record_exists(&ctx_state._db, comm_id.clone()).await?;
    let required_diss_auth = Authorization { authorize_record_id: comm_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
    AccessRightDbService { db: &ctx_state._db, ctx: &ctx }.is_authorized(&user_id, &required_diss_auth).await?;

    let access_r_db_ser = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx };
    let empty_auth_tb = "not_existing_table";

    let mut update_access_rule = match form_value.id.len() > 0 {
        false => {
            AccessRule {
                id: None,
                target_entity_id: comm_id.clone(),
                title: "".to_string(),
                authorization_required: Authorization {
                    authorize_record_id: Thing::from((empty_auth_tb, "not_existing_id")),
                    authorize_activity: "".to_string(),
                    authorize_height: 0,
                },
                price_amount: None,
                available_period_days: None,
                r_created: None,
                join_confirmation: None,
                join_redirect_url: None,
            }
        }
        true => {
            Thing::try_from(form_value.id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into access_rule Thing".to_string() }))?;
            access_r_db_ser.get(IdentIdName::Id(form_value.id)).await?
        }
    };

    if form_value.title.len() > 0 {
        update_access_rule.title = form_value.title;
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "title must have value".to_string() }));
    };

    if form_value.join_confirmation.trim().len() > 0 {
        update_access_rule.join_confirmation = Option::from(form_value.join_confirmation.trim().to_string());
    } else { update_access_rule.join_confirmation = None }

    if form_value.join_redirect_url.trim().len() > 0 {
        update_access_rule.join_redirect_url = Option::from(form_value.join_redirect_url.trim().to_string());
    } else { update_access_rule.join_redirect_url = None }

    if form_value.authorize_record_id_required.len() > 0 {
        let rec_id = Thing::try_from(form_value.authorize_record_id_required).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into rec_id Thing".to_string() }))?;
        let required_rec_auth = Authorization { authorize_record_id: rec_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
        AccessRightDbService { db: &ctx_state._db, ctx: &ctx }.is_authorized(&user_id, &required_rec_auth).await?;
        update_access_rule.authorization_required = Authorization { authorize_record_id: rec_id, authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(), authorize_height: form_value.authorize_height_required }
    }
    if update_access_rule.id.is_none() && update_access_rule.authorization_required.authorize_record_id.tb == empty_auth_tb {
        return Err(ctx.to_api_error(AppError::Generic { description: "no authorization set".to_string() }));
    }

    update_access_rule.price_amount = match form_value.price_amount.len() > 0 {
        true => form_value.price_amount.parse::<i32>().ok(),
        false => None,
    };

    if form_value.available_period_days.len() > 0 {
        let dur = form_value.available_period_days.parse::<u64>().unwrap_or(0);
        update_access_rule.available_period_days = match dur > 0 {
            true => Some(dur),
            false => None
        }
    }

    access_r_db_ser
        .create_update(update_access_rule)
        .await?;

    ctx.to_htmx_or_json_res::<AccessRuleForm>(get_form(State(ctx_state), ctx.clone(), Path(comm_id.to_raw()), Query(HashMap::new())).await?)
}


#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::entity::access_rule_entity::AccessRuleDbService;
    use crate::entity::authentication_entity::AuthType;
    use crate::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER, AUTH_ACTIVITY_VISITOR};
    use crate::entity::community_entitiy::CommunityDbService;
    use crate::entity::local_user_entity::{LocalUser, LocalUserDbService};
    use crate::routes::access_rule_routes::{AccessRuleForm, AccessRuleInput};
    use crate::routes::community_routes::{get_community, CommunityInput};
    use crate::routes::discussion_topic_routes::{DiscussionTopicItemsEdit, TopicInput};
    use sb_community::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::error::AppError;
    use sb_middleware::utils::db_utils::IdentIdName;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;
    use crate::entity::access_right_entity::AccessRightDbService;

    #[tokio::test]
    async fn display_access_rule_content() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let comm_name = "community_123";
        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name.clone().to_string(), title: "The Community Test".to_string() }).await;
        &create_response.assert_status_success();
        let created = &create_response.json::<CreatedResponse>();

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();

        let ctx = &Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
        let comm_db = CommunityDbService { db: &ctx_state._db, ctx: &ctx };
        let comm = comm_db.get(IdentIdName::Id(comm_id.clone().to_raw())).await;
        let comm_disc_id = comm.unwrap().main_discussion.unwrap();

        let create_response = server.post("/api/access-rule").json(&AccessRuleInput { id: "".to_string(), target_entity_id: comm_id.to_raw(), title: "Access Rule Register".to_string(), authorize_record_id_required: comm_id.to_raw(), authorize_activity_required: AUTH_ACTIVITY_VISITOR.to_string(), authorize_height_required: 1000, price_amount: "".to_string(), available_period_days: "".to_string(), join_confirmation: "".to_string(), join_redirect_url: "".to_string() }).await;
        &create_response.assert_status_success();
        let created = &create_response.json::<AccessRuleForm>();
        let ar_0 = created.access_rules.get(0).unwrap();
        let ar_id = ar_0.id.clone().unwrap();
        let ar_db = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx };

        let ar = ar_db.get(IdentIdName::Id(ar_id.to_raw()).into()).await.expect("access rule");
        assert_eq!(ar.id.clone().unwrap(), ar_id);

        let post_name = "post title Name 0".to_string();
        let create_post = server.post(&format!("/api/discussion/{comm_disc_id}/post")).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt111").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let post_name = "post title Name 1".to_string();
        let create_post = server.post(format!("/api/discussion/{comm_disc_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt111").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let comm_view = get_community(State(ctx_state.clone()), ctx.clone(), Path(comm_name.clone()), DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 2);


        let topic_title = "topic1".to_string();
        let topic_resp = server.post(format!("/api/discussion/{}/topic", comm_disc_id).as_str()).json(&TopicInput {
            id: "".to_string(),
            title: topic_title.clone(),
            hidden: None,
            access_rule_id: ar_id.to_raw(),
        }).await;
        &topic_resp.assert_status_success();
        let created = &topic_resp.json::<DiscussionTopicItemsEdit>();
        assert_eq!(&created.community_id, &comm_id);
        assert_eq!(&created.topics.get(0).unwrap().title, &topic_title);
        let topic1_id = created.topics.get(0).unwrap().id.clone();

        let post_name = "post title Name 2".to_string();
        let create_post = server.post(format!("/api/discussion/{comm_disc_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt111").add_text("topic_id", topic1_id.clone().unwrap().to_raw())).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let comm_view = get_community(State(ctx_state.clone()), ctx.clone(), Path(comm_name.clone()), DiscussionParams {
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 3);

        // check view with admin
        let comm_view = get_community(State(ctx_state.clone()), ctx.clone(), Path(comm_name.clone()), DiscussionParams {
            topic_id: topic1_id.clone(),
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 1);
        let post0 = posts.get(0).unwrap();
        let post_access_rule = post0.access_rule.clone().unwrap();
        assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
        assert_eq!(post0.viewer_access_rights.len() == 1, true);
        let post_viewer_access_rights = post0.viewer_access_rights.get(0).unwrap();
        assert_eq!(post_viewer_access_rights.authorize_activity, AUTH_ACTIVITY_OWNER.to_string());
        assert_eq!(post_viewer_access_rights.ge_equal_ident(&post_access_rule.authorization_required).expect("ok"), true);
        assert_eq!(post_access_rule.authorization_required, ar.authorization_required);

        // check view with no user
        let ctx_no_user = Ctx::new(Err(AppError::AuthFailNoJwtCookie), Uuid::new_v4(), false);
        let comm_view = get_community(State(ctx_state.clone()), ctx_no_user, Path(comm_name.clone()), DiscussionParams {
            topic_id: topic1_id.clone(),
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 1);
        let post0 = posts.get(0).unwrap();
        let post_access_rule = post0.access_rule.clone().unwrap();
        assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
        assert_eq!(post0.viewer_access_rights.len() == 0, true);
        assert_eq!(post_access_rule.authorization_required, ar.authorization_required);

        // check view with low access user
        let user_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
        let new_user_id = user_service.create(LocalUser {
            id: None,
            username: "visitor".to_string(),
            email: None,
            image_uri: None,
        }, AuthType::PASSWORD(Some("visitor".to_string()))).await.expect("local user");

        AccessRightDbService { db: &ctx_state._db, ctx }.authorize(Thing::try_from(new_user_id.clone()).unwrap(), Authorization {
            authorize_record_id: comm_id.clone(),
            authorize_activity: AUTH_ACTIVITY_VISITOR.to_string(),
            authorize_height: 1,
        }, None).await.expect("authorized");

        let ctx_no_user = Ctx::new(Ok(new_user_id), Uuid::new_v4(), false);
        let comm_view = get_community(State(ctx_state.clone()), ctx_no_user, Path(comm_name.clone()), DiscussionParams {
            topic_id: topic1_id.clone(),
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 1);
        let post0 = posts.get(0).unwrap();
        let post_access_rule = post0.access_rule.clone().unwrap();
        assert_eq!(post_access_rule.id.clone().unwrap(), ar_id);
        assert_eq!(post0.viewer_access_rights.len() == 1, true);
        let post_viewer_access_rights = post0.viewer_access_rights.get(0).unwrap();
        assert_eq!(post_viewer_access_rights.authorize_activity, AUTH_ACTIVITY_VISITOR.to_string());
        assert_eq!(post_viewer_access_rights.ge_equal_ident(&post_access_rule.authorization_required).expect("ok"), false);
        assert_eq!(post_access_rule.authorization_required, ar.authorization_required);
    }
}

