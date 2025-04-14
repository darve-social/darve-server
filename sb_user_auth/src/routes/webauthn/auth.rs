use axum::extract::State;
use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use log::{error, info};
use surrealdb::sql::{
    Thing};
use tower_cookies::{
     Cookies};
use tower_sessions::Session;
// 1. Import the prelude - this contains everything needed for the server to function.
use webauthn_rs::prelude::*;

use crate::entity::authentication_entity::{AuthType, AuthenticationDbService};
use crate::entity::local_user_entity::{LocalUser, LocalUserDbService};
use crate::routes::webauthn::error::WebauthnError;
use crate::routes::webauthn::startup::AppState;
use sb_middleware::ctx::Ctx;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::cookie_utils;
use sb_middleware::utils::db_utils::{IdentIdName, UsernameIdent};
use sb_middleware::utils::string_utils::get_string_thing;
use crate::routes::register_routes::validate_username;
/*
 * Webauthn RS auth handlers.
 * These files use webauthn to process the data received from each route, and are closely tied to axum
 */

// 2. The first step a client (user) will carry out is requesting a credential to be
// registered. We need to provide a challenge for this. The work flow will be:
//
//          ┌───────────────┐     ┌───────────────┐      ┌───────────────┐
//          │ Authenticator │     │    Browser    │      │     Site      │
//          └───────────────┘     └───────────────┘      └───────────────┘
//                  │                     │                      │
//                  │                     │     1. Start Reg     │
//                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│
//                  │                     │                      │
//                  │                     │     2. Challenge     │
//                  │                     │◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┤
//                  │                     │                      │
//                  │  3. Select Token    │                      │
//             ─ ─ ─│◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│                      │
//  4. Verify │     │                     │                      │
//                  │  4. Yield PubKey    │                      │
//            └ ─ ─▶│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶                      │
//                  │                     │                      │
//                  │                     │  5. Send Reg Opts    │
//                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│─ ─ ─
//                  │                     │                      │     │ 5. Verify
//                  │                     │                      │         PubKey
//                  │                     │                      │◀─ ─ ┘
//                  │                     │                      │─ ─ ─
//                  │                     │                      │     │ 6. Persist
//                  │                     │                      │       Credential
//                  │                     │                      │◀─ ─ ┘
//                  │                     │                      │
//                  │                     │                      │
//
// In this step, we are responding to the start reg(istration) request, and providing
// the challenge to the browser.

pub async fn start_register(
    State(CtxState { _db, .. }): State<CtxState>,
    Extension(app_state): Extension<AppState>,
    session: Session,
    _cookies: Cookies,
    ctx: Ctx,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, WebauthnError> {
    println!("Start register passkey");
    // We get the username from the URL, but you could get this via form submission or
    // some other process. In some parts of Webauthn, you could also use this as a "display name"
    // instead of a username. Generally you should consider that the user *can* and *will* change
    // their username at any time.

    // Since a user's username could change at anytime, we need to bind to a unique id.
    // We use uuid's for this purpose, and you should generate these randomly. If the
    // username does exist and is found, we can match back to our unique id. This is
    // important in authentication, where presented credentials may *only* provide
    // the unique id, and not the username!

    let user_db_service = &LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let exclude_credentials: Option<Vec<CredentialID>> = None;
    #[allow(unused_assignments)]
    let mut register_user_ident = None;
    let username = username;
    validate_username(&username).map_err(|_| WebauthnError::WebauthnApiError("username not valid".to_string()))?;
    let logged_user_id = ctx.user_id().ok();

    if logged_user_id.is_none() {
        let username_is_available = user_db_service
            .exists(UsernameIdent(username.clone().trim().to_string()).into())
            .await?
            .is_none();

        if !username_is_available {
            return Err(WebauthnError::UserExists);
        }
        register_user_ident = Some(username.clone());
    } else {
        // user is logged in and has user id
        // registerUuid = RegisterUserUuid::Existing(Uuid::new_v4(), username_user_id);
        let l_user_id = get_string_thing(logged_user_id.clone().unwrap())
            .map_err(|_| WebauthnError::CorruptSession)?;
        let local_user = user_db_service.get(IdentIdName::Id(l_user_id)).await.map_err(|_|WebauthnError::UserHasNoCredentials)?;
        if &local_user.username != &username {
            return Err(WebauthnError::UserNotFound);
        }
        //TODO loggedin username exists, collect passkey auths from all db passkey CredentialIDs authentication records in ids
        // username = local_user.username;
        // register_user_ident = Some(username.clone());
        // TODO continue implementation ...
        return Err(WebauthnError::WebauthnNotImplemented);
        // exclude_credentials = ...
    }

    let register_user_ident = match register_user_ident {
        None => return Err(WebauthnError::Unknown),
        Some(user_ident) => user_ident,
    };

    /*let user_unique_id = {
        let users_guard = app_state.users.lock().await;
        users_guard
            .user_ident_to_uuid
            .get(&register_user_ident)
            .copied()
            .unwrap_or_else(Uuid::new_v4)
    };*/
    let registration_uuid = Uuid::new_v4();

    // Remove any previous registrations that may have occured from the session.
    session.remove_value("reg_state").await?;

    // If the user has any other credentials, we exclude these here so they can't be duplicate registered.
    // It also hints to the browser that only new credentials should be "blinked" for interaction.
    /* taken from db
    let exclude_credentials = {
        let users_guard = app_state.users.lock().await;
        users_guard
            .keys
            .get(&Uuid::from_str(&*user_unique_id.unwrap()).unwrap())
            .map(|keys| keys.iter().map(|sk| sk.cred_id().clone()).collect())
    };*/

    let res = match app_state.webauthn.start_passkey_registration(
        registration_uuid,
        &register_user_ident,
        &username,
        exclude_credentials,
    ) {
        Ok((ccr, reg_state)) => {

            // Note that due to the session store in use being a server side memory store, this is
            // safe to store the reg_state into the session since it is not client controlled and
            // not open to replay attacks. If this was a cookie store, this would be UNSAFE.
            session
                .insert(
                    "reg_state",
                    (register_user_ident, registration_uuid, reg_state),
                )
                .await
                .expect("Failed to insert");
            info!("Registration Successful!");
            Json(ccr)
        }
        Err(e) => {
            info!("challenge_register -> {:?}", e);
            return Err(WebauthnError::Unknown);
        }
    };
    Ok(res)
}

// 3. The browser has completed it's steps and the user has created a public key
// on their device. Now we have the registration options sent to us, and we need
// to verify these and persist them.

pub async fn finish_register(
    State(CtxState { _db, .. }): State<CtxState>,
    _cookies: Cookies,
    ctx: Ctx,
    Extension(app_state): Extension<AppState>,
    session: Session,
    Json(reg): Json<RegisterPublicKeyCredential>,
) -> Result<impl IntoResponse, WebauthnError> {
    // TODO if username exists and user has jwt for that user add new authentication with sk.cred_id().clone()
    // if not exists create new user and add authentication
    let (register_user_ident, _registration_uuid, reg_state): (String, String, PasskeyRegistration) =
        match session.get("reg_state").await? {
            Some((register_user_ident, registration_uuid, reg_state)) => {
                (register_user_ident, registration_uuid, reg_state)
            }
            None => {
                error!("Failed to get session");
                return Err(WebauthnError::CorruptSession);
            }
        };

    session.remove_value("reg_state").await?;

    let valid_passkey = app_state
        .webauthn
        .finish_passkey_registration(&reg, &reg_state)
        .ok();
    /*{
        Ok(sk) => {
            /*let mut users_guard = app_state.users.lock().await;

            let b64CredentialID = STANDARD.encode(sk.cred_id().to_vec());
            println!("222PKKKK={:?}", b64CredentialID);
            // println!("hex={:x?}", f);
            //TODO: This is where we would store the credential in a db, or persist them in some other way.
            users_guard
                .keys
                .entry(user_unique_id)
                .and_modify(|keys| keys.push(sk.clone()))
                .or_insert_with(|| vec![sk.clone()]);

            users_guard.user_ident_to_uuid.insert(username, user_unique_id);*/
            Some(sk)
            // StatusCode::OK
        }
        Err(e) => {
            error!("challenge_register -> {:?}", e);
            None
            // StatusCode::BAD_REQUEST
        }
    };*/

    if valid_passkey.is_none() {
        return Ok(StatusCode::BAD_REQUEST);
    }

    let user_db_service = &LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let logged_user_id = ctx.user_id().ok();

    return if logged_user_id.is_none() {
        // user is making passkey for new username
        let username = register_user_ident.clone().trim().to_string();
        validate_username(&username).map_err(|_| WebauthnError::WebauthnApiError("username not valid".to_string()))?;

        let username_is_available = user_db_service
            .exists(UsernameIdent(username).into())
            .await?
            .is_none();

        if !username_is_available {
            return Err(WebauthnError::UserExists);
        }
        user_db_service
            .create(
                LocalUser {
                    id: None,
                    username: register_user_ident,
                    full_name: None,
                    birth_date: None,
                    phone: None,
                    email: None,
                    bio: None,
                    social_links: None,
                    image_uri: None,
                },
                AuthType::PASSKEY(
                    Some(valid_passkey.as_ref().unwrap().cred_id().clone()),
                    valid_passkey,
                ),
            )
            .await?;
         Ok(StatusCode::OK)

    } else {
        // user is making passkey for existing username
        let user_id: Thing = get_string_thing(logged_user_id.unwrap()).map_err(|_| WebauthnError::CorruptSession)?;
        let username = register_user_ident.clone().trim().to_string();
        let local_user = user_db_service.get(IdentIdName::Id(user_id)).await.map_err(|_| WebauthnError::UserNotFound)?;

        if local_user.username != username {
            return Err(WebauthnError::UserNotFound);
        }

        // TODO continue - add new auth for user...
        Err(WebauthnError::WebauthnNotImplemented)
        /*users_guard
        .keys
        .entry(user_unique_id)
        .and_modify(|keys| keys.push(sk.clone()))
        .or_insert_with(|| vec![sk.clone()]);*/

    };



    // Ok(StatusCode::NOT_ACCEPTABLE)
}

// 4. Now that our public key has been registered, we can authenticate a user and verify
// that they are the holder of that security token. The work flow is similar to registration.
//
//          ┌───────────────┐     ┌───────────────┐      ┌───────────────┐
//          │ Authenticator │     │    Browser    │      │     Site      │
//          └───────────────┘     └───────────────┘      └───────────────┘
//                  │                     │                      │
//                  │                     │     1. Start Auth    │
//                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│
//                  │                     │                      │
//                  │                     │     2. Challenge     │
//                  │                     │◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┤
//                  │                     │                      │
//                  │  3. Select Token    │                      │
//             ─ ─ ─│◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│                      │
//  4. Verify │     │                     │                      │
//                  │    4. Yield Sig     │                      │
//            └ ─ ─▶│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶                      │
//                  │                     │    5. Send Auth      │
//                  │                     │        Opts          │
//                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│─ ─ ─
//                  │                     │                      │     │ 5. Verify
//                  │                     │                      │          Sig
//                  │                     │                      │◀─ ─ ┘
//                  │                     │                      │
//                  │                     │                      │
//
// The user indicates the wish to start authentication and we need to provide a challenge.

pub async fn start_authentication(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Extension(app_state): Extension<AppState>,
    session: Session,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, WebauthnError> {
    info!("Start Authentication");
    // We get the username from the URL, but you could get this via form submission or
    // some other process.

    // if username exists get all user passkey authentications (saves in row in authentication table)

    // Remove any previous authentication that may have occured from the session.
    session.remove_value("auth_state").await?;
    /*
    // Get the set of keys that the user possesses
    let users_guard = app_state.users.lock().await;

    // Look up their unique id from the username
    let user_unique_id = users_guard
        .user_ident_to_uuid
        .get(&username)
        .copied()
        .ok_or(WebauthnError::UserNotFound)?;

    let allow_credentials = users_guard
        .keys
        .get(&user_unique_id)
        .ok_or(WebauthnError::UserHasNoCredentials)?;*/

    let user_db_service = &LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let auth_db_service = &AuthenticationDbService {
        db: &_db,
        ctx: &ctx,
    };

    let exists_id = user_db_service
        .exists(UsernameIdent(username).into())
        .await?;

    if exists_id.is_none() {
        return Err(WebauthnError::UserNotFound);
    }

    let allow_credentials: Vec<Passkey> = auth_db_service
        .get_by_auth_type(exists_id.clone().unwrap(), AuthType::PASSKEY(None, None))
        .await?
        .into_iter()
        .filter_map(|auth| match auth.passkey_json {
            None => None,
            Some(pk_str) => {
                let pk_res = serde_json::from_str::<Passkey>(&pk_str);
                if pk_res.is_err() {
                    println!("ERROR parsing into json PASSKEY={}", pk_str);
                }
                pk_res.ok()
            }
        })
        .collect();

    if allow_credentials.len() < 1 {
        return Err(WebauthnError::UserHasNoCredentials);
    }

    let res = match app_state
        .webauthn
        .start_passkey_authentication(&*allow_credentials)
    {
        Ok((rcr, auth_state)) => {
            // Drop the mutex to allow the mut borrows below to proceed
            // drop(users_guard);

            // Note that due to the session store in use being a server side memory store, this is
            // safe to store the auth_state into the session since it is not client controlled and
            // not open to replay attacks. If this was a cookie store, this would be UNSAFE.
            session
                .insert("auth_state", (exists_id.unwrap(), auth_state))
                .await
                .expect("Failed to insert");
            Json(rcr)
        }
        Err(e) => {
            info!("challenge_authenticate -> {:?}", e);
            return Err(WebauthnError::Unknown);
        }
    };
    Ok(res)
}

// 5. The browser and user have completed their part of the processing. Only in the
// case that the webauthn authenticate call returns Ok, is authentication considered
// a success. If the browser does not complete this call, or *any* error occurs,
// this is an authentication failure.

pub async fn finish_authentication(
    State(CtxState { _db, key_enc, jwt_duration, .. }): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    Extension(app_state): Extension<AppState>,
    session: Session,
    Json(auth): Json<PublicKeyCredential>,
) -> Result<impl IntoResponse, WebauthnError> {
    let (user_unique_id, auth_state): (String, PasskeyAuthentication) = session
        .get("auth_state")
        .await?
        .ok_or(WebauthnError::CorruptSession)?;
    let u_uniq_id =
        get_string_thing(user_unique_id.clone()).map_err(|_| WebauthnError::CorruptSession)?;

    let _ = session.remove_value("auth_state").await.is_ok();

    let res = match app_state
        .webauthn
        .finish_passkey_authentication(&auth, &auth_state)
    {
        Ok(auth_result) => {
            // TODO get credential by localUserId:{type:PASSKEY(auth_result.cred_id())}

            let user_db_service = &LocalUserDbService {
                db: &_db,
                ctx: &ctx,
            };
            let auth_db_service = &AuthenticationDbService {
                db: &_db,
                ctx: &ctx,
            };

            let exists_id = user_db_service.exists(IdentIdName::Id(u_uniq_id)).await?;

            if exists_id.is_none() {
                return Err(WebauthnError::UserNotFound);
            }

            let user_id = auth_db_service
                .authenticate(
                    &ctx,
                    user_unique_id,
                    AuthType::PASSKEY(Some(auth_result.cred_id().clone()), None),
                )
                .await;

            if user_id.is_err() {
                return Err(WebauthnError::UserHasNoCredentials);
            }

            cookie_utils::issue_login_jwt(&key_enc, cookies, exists_id, jwt_duration);
            // let mut users_guard = app_state.users.lock().await;
            // Update the credential counter, if possible.
            /*users_guard
            .keys
            .get_mut(&user_unique_id)
            .map(|keys| {
                keys.iter_mut().for_each(|sk| {
                    // This will update the credential if it's the matching
                    // one. Otherwise it's ignored. That is why it is safe to
                    // iterate this over the full list.
                    sk.update_credential(&auth_result);
                })
            })
            .ok_or(WebauthnError::UserHasNoCredentials)?;*/
            StatusCode::OK
        }
        Err(e) => {
            println!("challenge_register -> {:?}", e);
            StatusCode::BAD_REQUEST
        }
    };
    info!("Authentication Successful!");
    Ok(res)
}
