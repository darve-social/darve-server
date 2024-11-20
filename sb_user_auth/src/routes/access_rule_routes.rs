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
use sb_middleware::utils::string_utils::get_string_thing;

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
    pub access_gain_action_confirmation: String,
    pub access_gain_action_redirect_url: String,
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
            access_gain_action_confirmation: None,
            access_gain_action_redirect_url: None,
            r_created: None,
            price_amount: None,
        },
        Some(id) => {
            let id= get_string_thing(id)?;
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

    let comm_id = get_string_thing(form_value.target_entity_id.clone())?;
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
                access_gain_action_confirmation: None,
                access_gain_action_redirect_url: None,
            }
        }
        true => {
            access_r_db_ser.get(IdentIdName::Id(get_string_thing(form_value.id.clone())?)).await?
        }
    };

    if form_value.title.len() > 0 {
        update_access_rule.title = form_value.title;
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "title must have value".to_string() }));
    };

    if form_value.access_gain_action_confirmation.trim().len() > 0 {
        update_access_rule.access_gain_action_confirmation = Option::from(form_value.access_gain_action_confirmation.trim().to_string());
    } else { update_access_rule.access_gain_action_confirmation = None }

    if form_value.access_gain_action_redirect_url.trim().len() > 0 {
        update_access_rule.access_gain_action_redirect_url = Option::from(form_value.access_gain_action_redirect_url.trim().to_string());
    } else { update_access_rule.access_gain_action_redirect_url = None }

    if form_value.authorize_record_id_required.len() > 0 {
        let rec_id = get_string_thing(form_value.authorize_record_id_required)?;
        let required_rec_auth = Authorization { authorize_record_id: rec_id.clone(), authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 1 };
        AccessRightDbService { db: &ctx_state._db, ctx: &ctx }.is_authorized(&user_id, &required_rec_auth).await?;
        update_access_rule.authorization_required = Authorization { authorize_record_id: rec_id, authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(), authorize_height: form_value.authorize_height_required }
    }
    if update_access_rule.id.is_none() && update_access_rule.authorization_required.authorize_record_id.tb == empty_auth_tb {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "no authorization set".to_string() }));
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
