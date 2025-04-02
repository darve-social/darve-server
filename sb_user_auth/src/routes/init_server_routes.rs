use std::fmt::Debug;

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
// use serde_json;
use surrealdb::sql::Thing;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use validator::Validate;

use crate::entity::access_right_entity::AccessRightDbService;
use crate::entity::authorization_entity::{
    get_root_auth_rec_name, Authorization, AUTH_ACTIVITY_OWNER,
};
use crate::entity::local_user_entity::LocalUserDbService;
use crate::routes::register_routes::{register_user, RegisterInput};
use crate::utils::template_utils::ProfileFormPage;
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::string_utils::get_string_thing;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/init", get(get_init_form))
        .route("/init", post(post_init_form))
        .route("/backup", get(backup))
        .with_state(state)
}

#[derive(Template)] // this will generate the code...
#[template(path = "nera2/init_form.html")] // using the template in this path, relative
                                           // to the `templates` dir in the crate root
struct InitServerForm {
    // the name of the struct can be anything
}

async fn get_init_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Response> {
    println!("->> {:<12} - init _server_display - ", "HANDLER");

    if !can_init(&_db, &ctx).await {
        Err(ctx.to_ctx_error(AppError::Generic {
            description: "Already initialized".to_string(),
        }))
    } else {
        Ok(ProfileFormPage::new(Box::new(InitServerForm {}), None, None, None).into_response())
    }
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct InitServerData {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub init_pass: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub username: String,
    #[validate(length(min = 12, message = "Min 12 characters"))]
    pub password: String,
    #[validate(email(message = "Email"))]
    pub email: String,
}

async fn backup(
    State(CtxState {
        _db,
        is_development,
        ..
    }): State<CtxState>,
    _ctx: Ctx,
) -> Response {
    if !is_development {
        return (StatusCode::OK, "not development").into_response();
    }
    let mut backup = _db.export(()).await.unwrap();
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("/Users/mac02/dev/DB_BACKUP.surql")
        .await
        .unwrap();
    // println!("DB BBACC={:?}", file.metadata().unwrap());
    while let Some(result) = backup.next().await {
        match result {
            Ok(bytes) => {
                file.write_all(bytes.as_slice()).await.unwrap();
            }
            Err(error) => {
                // Handle the export error
                println!("ERRRRRR {}", error);
            }
        }
    }
    (StatusCode::OK, "created backup").into_response()
}
async fn post_init_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    payload: JsonOrFormValidated<InitServerData>,
) -> CtxResult<Html<String>> {
    println!(
        "->> {:<12} - init save user {:#?}- {}",
        "HANDLER", payload, ctx.is_htmx
    );

    if !can_init(&_db, &ctx).await {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "Already initialized".to_string(),
        }));
    }

    let reg_input = RegisterInput {
        username: payload.0.username,
        password: payload.0.password.clone(),
        password1: payload.0.password.clone(),
        email: Some(payload.0.email),
        bio: None,
        full_name: None,
        image_uri: None,
        next: None,
    };

    let created_user = register_user(&_db, &ctx, &reg_input).await?;
    let auth_thing = Thing::from((get_root_auth_rec_name(), "0".to_string()));
    let authorization = Authorization::new(auth_thing.into(), AUTH_ACTIVITY_OWNER.to_string(), 99)?;

    let aright_db_service = &AccessRightDbService {
        db: &_db,
        ctx: &ctx,
    };
    let user_rec_id = get_string_thing(created_user.clone().id)?;
    aright_db_service
        .authorize(user_rec_id, authorization, None)
        .await?;
    ctx.to_htmx_or_json(&created_user)
}

async fn can_init(_db: &Db, ctx: &Ctx) -> bool {
    LocalUserDbService { ctx, db: &_db }
        .users_len()
        .await
        .map(|result| result == 0)
        .unwrap_or(false)
}
