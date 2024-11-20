use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use axum_htmx::HX_REDIRECT;
use futures::stream::Stream as FStream;
use futures::{FutureExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

use crate::entity::access_right_entity::AccessRightDbService;
use crate::entity::access_rule_entity::{AccessRule, AccessRuleDbService};
use crate::entity::local_user_entity::LocalUserDbService;
use crate::entity::access_gain_action_entitiy::{AccessGainAction, AccessGainActionDbService, AccessGainActionStatus, AccessGainActionType};
use crate::routes::register_routes::{display_register_form, display_register_page};
use crate::utils::askama_filter_util::filters;
use crate::utils::template_utils::ProfileFormPage;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/access-rule/:access_rule_id/join", get(access_gain_action_page))
        .route("/api/access-rule/:access_rule_id/join", get(access_gain_action_form))
        .route("/api/join/access-rule", post(save_access_gain_action))
        .with_state(state)
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/access_gain_action_form.html")]
pub struct AccessGainActionForm {
    pub access_rule: AccessRule,
    pub next: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AccessGainActionInput {
    #[validate(email)]
    pub email: String,
    pub access_rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

#[derive(Deserialize)]
pub struct AccessRuleView {
    pub(crate) id: Thing,
    pub(crate) target_entity_id: Thing,
    pub(crate) access_gain_action_redirect_url: Option<String>,
    pub(crate) access_gain_action_confirmation: Option<String>,
}

impl ViewFieldSelector for AccessRuleView {
    fn get_select_query_fields(ident: &IdentIdName) -> String {
        "id, target_entity_id, access_gain_action_redirect_url, access_gain_action_confirmation".to_string()
    }
}


async fn access_gain_action_page(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(access_rule_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    if ctx.user_id().is_err() {
        let mut qry: HashMap<String, String> = HashMap::new();
        qry.insert("next".to_string(), format!("/access-rule/{access_rule_id}/join"));
        // TODO add cookie when loggedin so app knows if should go to registration
        return Ok(display_register_page(ctx, Query(qry)).await?.into_response());
    }
    let user = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user().await?;
    let access_rule = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx }.get(IdentIdName::Id(get_string_thing(access_rule_id)?)).await?;
    if access_rule.price_amount.unwrap_or(0) > 0 {
        return Ok(Redirect::temporary(format!("/api/stripe/access-rule/{}", access_rule.id.expect("is saved").to_raw()).as_str()).into_response());
    }

    Ok(ProfileFormPage::new(Box::new(AccessGainActionForm {
        access_rule,
        next: qry.remove("next"),
        email: user.email,
    }), None, None).into_response())
}

async fn access_gain_action_form(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(access_rule_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    if ctx.user_id().is_err() {
        let mut qry: HashMap<String, String> = HashMap::new();
        qry.insert("next".to_string(), format!("/access-rule/{access_rule_id}/join"));
        // TODO add cookie when loggedin so app knows if should go to registration
        return Ok(display_register_form(ctx, Query(qry)).await?.into_response());
    }
    let user = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user().await?;
    let access_rule = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx }.get(IdentIdName::Id(get_string_thing(access_rule_id)?)).await?;
    if access_rule.price_amount.unwrap_or(0) > 0 {
        return Ok(Redirect::temporary(format!("/api/stripe/access-rule/{}", access_rule.id.expect("is saved").to_raw()).as_str()).into_response());
    }
    let response_str = AccessGainActionForm {
        access_rule,
        next: qry.remove("next"),
        email: user.email,
    }.render().map_err(|e| ctx.to_ctx_error(AppError::Generic { description: e.to_string() }))?;
    Ok((StatusCode::OK, Html(response_str)).into_response())
}

async fn save_access_gain_action(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    JsonOrFormValidated(form_value): JsonOrFormValidated<AccessGainActionInput>,
) -> CtxResult<Response> {
    let user_id = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user_thing().await?;

    let local_user_db_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
    let mut user = local_user_db_service.get(IdentIdName::Id(user_id.clone())).await?;
    if user.email.is_none() {
        user.email = Option::from(form_value.email.to_lowercase());
        user = local_user_db_service.update(user).await?;
    }
    if user.email.is_none() || user.email.unwrap().to_lowercase().ne(&form_value.email.to_lowercase()) {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "Profile email different than provided".to_string() }));
    }

    let access_rule = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx }.get_view::<AccessRuleView>(IdentIdName::Id(get_string_thing(form_value.access_rule_id)?)).await?;

    let access_gain_action_db_service = AccessGainActionDbService { db: &ctx_state._db, ctx: &ctx };

    // TODO start with failed and check against access_gain_action_confirmation requirements
    let mut action_status = AccessGainActionStatus::Complete;
    let mut j_action = if access_rule.access_gain_action_confirmation.unwrap_or("".to_string()).len() > 1 {
        let access_rule_pending = Some(access_rule.id.clone());
        action_status = AccessGainActionStatus::Pending;

        let already_pending = access_gain_action_db_service.get(IdentIdName::ColumnIdentAnd(
            vec![
                IdentIdName::ColumnIdent {
                    rec: true,
                    column: "local_user".to_string(),
                    val: user_id.to_raw(),
                }, IdentIdName::ColumnIdent {
                    rec: true,
                    column: "access_rule_pending".to_string(),
                    val: access_rule_pending.clone().unwrap().to_raw(),
                }, IdentIdName::ColumnIdent {
                    rec: false,
                    column: "action_status".to_string(),
                    val: action_status.to_string(),
                },
            ]
        )).await;
        //return existing pending or create new
        if already_pending.is_ok() {
            already_pending.unwrap()
        } else {
            access_gain_action_db_service.create_update(create_access_gain_action(&user_id, access_rule_pending, action_status)).await?
        }
    } else {
        access_gain_action_db_service.create_update(create_access_gain_action(&user_id, None, AccessGainActionStatus::Complete)).await?
    };

    // TODO add func. - access_gain_action status is complete by checking some values or must be confirmed by admin
    // let mut j_action = join_action_db_service.create_update(create_joint_action(&user_id, access_rule_pending, action_status)).await?;

    let response = match j_action.action_status {
        AccessGainActionStatus::Complete => {
            let a_right = AccessRightDbService { ctx: &ctx, db: &ctx_state._db }.add_paid_access_right(user_id.clone(), access_rule.id.clone(), j_action.id.clone().expect("must be saved already, having id")).await?;
            j_action.access_rights = Option::from(vec![a_right.id.expect("a_right must be saved")]);
            let j_action = access_gain_action_db_service.create_update(j_action).await?;

            let res = CreatedResponse { success: true, id: j_action.id.unwrap().to_raw(), uri: None };
            let mut res = ctx.to_htmx_or_json_res::<CreatedResponse>(res).into_response();
            // TODO add next param

            let next = match access_rule.access_gain_action_redirect_url {
                None => {
                    if access_rule.target_entity_id.tb == "community" {
                        format!("/community/{}", access_rule.target_entity_id)
                    } else {
                        format!("/uid/{}", user_id)
                    }
                }
                Some(url) => url
            };
            res.headers_mut().append(HX_REDIRECT, next.parse().unwrap());
            res
        }
        AccessGainActionStatus::Failed => {
            (StatusCode::OK, "Error vailidating provided form data").into_response()
        }
        AccessGainActionStatus::Pending => {
            let next = match access_rule.access_gain_action_redirect_url {
                None => {
                    format!("/uid/{}", user_id)
                }
                Some(url) => url
            };
            let mut res = (StatusCode::OK, format!("Success, you will be notified when admin accepts your request.\
            <a href='{}'>Click here to continue</a>", next)).into_response();
            res
        }
    };
    Ok(response)
}

fn create_access_gain_action(user_id: &Thing, mut access_rule_pending: Option<Thing>, mut action_status: AccessGainActionStatus) -> AccessGainAction {
    AccessGainAction {
        id: None,
        external_ident: None,
        access_rule_pending,
        access_rights: None,
        local_user: Option::from(user_id.clone()),
        action_type: AccessGainActionType::LocalUser,
        action_status,
        r_created: None,
        r_updated: None,
    }
}
