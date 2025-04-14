use axum::extract::{FromRequest, Query, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
    Form, Json, RequestExt,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use surrealdb::sql::Thing;
use validator::{Validate, ValidationErrors};

use crate::error::{to_err_html, ErrorResponseBody};
use crate::mw_ctx::CtxState;

/*
#[derive(Debug)]
pub struct HostDomainId(String);

#[async_trait]
impl FromRequestParts<CtxState> for HostDomainId

{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &CtxState) -> Result<Self, Self::Rejection> {
        use axum::RequestPartsExt;

        let hostName = Host::from_request_parts(parts, state)
            .await
            .map_err(|err| {
                dbg!(&err);
                err.into_response()
            }
            )?.0.clone();

        let cookies = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|err| {
                dbg!(&err);
                err.into_response()
            }
            )?;

        let DOMAIN_ID_COOKIE = "domainId";
        let cachedIdent = cookies.get(DOMAIN_ID_COOKIE);
        if let Some(cachedDomainId) = cachedIdent {
            let mut parsedIter = cachedDomainId.value().split_whitespace().into_iter();
            if let (Some(cachedName), Some(cachedId)) = (parsedIter.next(), parsedIter.next()) {
                if cachedName == &hostName {
                    println!("domainID found in cookies");
                    return Ok(Self(cachedId.to_string()));
                }
            }
        }

        let Extension(ctx) = parts.extract::<Extension<Ctx>>()
            .await
            .map_err(|err| {
                dbg!(&err);
                err.into_response()
            }
            )?;
        let domainService = DomainDbService { db: &state._db, ctx: &ctx };
        let domain = domainService.get(hostName.clone()).await.map_err(|e| {
            println!("extractor_utils - ERROR host domain NOT FOUND in db");
            e.into_response()
        })?;
        let domainId = domain.id.unwrap().to_string();
        let newCookie = CookieBuilder::new(DOMAIN_ID_COOKIE, format!("{hostName} {domainId}"))
            .domain(hostName)
            .http_only(true)
            .max_age(Duration::days(30))
            .build();
        cookies.add(newCookie);
        // dbg!(&domain);
        Ok(Self(domainId))
    }
}*/

/*#[derive(Debug, Clone, Copy, Default)]
pub struct JsonOrHtmxForm<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for JsonOrHtmxForm<T>
    where
        T: DeserializeOwned,
        S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(mut req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();
        let is_htmx = HxRequest::from_request_parts(&mut parts, state  ).await.expect("get hx request");
        let req = Request::from_parts(parts, body);
        if let HxRequest(is_htmx) = is_htmx {
            if is_htmx {
                let res:Form<T> = Form::from_request(req, state).await.map_err(|err| {
                    dbg!(&err);
                    err.into_response()
                }
                )?;
                if let Form(res) = res {
                    return Ok(JsonOrHtmxForm(res));
                }
                return Err((StatusCode::BAD_REQUEST, "can not parse htmx form values".to_string()).into_response());
            }
        }
        let res: Json<T> = Json::from_request(req, state).await.map_err(|err| {
            dbg!(&err);
            err.into_response()
        }
        )?;
        if let Json(res) = res {
            return Ok(JsonOrHtmxForm(res));
        }
        return Err((StatusCode::BAD_REQUEST, "can not parse post json values".to_string()).into_response());
    }
}*/

#[derive(Debug)]
pub struct JsonOrFormValidated<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for JsonOrFormValidated<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<()>,
    Form<T>: FromRequest<()>,
    T: DeserializeOwned + Validate + 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                let validation: Result<(), ValidationErrors> = payload.validate();
                validation.map_err(|err| {
                    {
                        let body: String = ErrorResponseBody::new(err.to_string(), None).into();
                        (StatusCode::BAD_REQUEST, body)
                    }
                    .into_response()
                })?;
                return Ok(Self(payload));
            }

            if content_type.starts_with("application/x-www-form-urlencoded") {
                // htmx request
                let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                payload.validate().map_err(|err| {
                    { (StatusCode::BAD_REQUEST, to_err_html(err.to_string())) }.into_response()
                })?;
                return Ok(Self(payload));
            }
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

// TODO make DiscussionParams more generic so can be used elswhere for pagination like wallet routes
#[derive(Debug, Deserialize, Clone)]
pub struct DiscussionParams {
    pub topic_id: Option<Thing>,
    pub start: Option<i32>,
    pub count: Option<i8>,
}
#[derive(Deserialize)]
struct DiscParamsRaw {
    topic_id: Option<String>,
    start: Option<i32>,
    count: Option<i8>,
}
#[async_trait]
impl FromRequestParts<CtxState> for DiscussionParams {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &CtxState,
    ) -> Result<Self, Self::Rejection> {
        let qry: Query<DiscParamsRaw> =
            Query::from_request_parts(parts, state)
                .await
                .map_err(|err| {
                    dbg!(&err);
                    err.into_response()
                })?;

        let Query(dp_raw) = qry;

        Ok(DiscussionParams {
            topic_id: match dp_raw.topic_id {
                Some(tid_string) => Thing::try_from(tid_string).ok(),
                _ => None,
            },
            count: dp_raw.count,
            start: dp_raw.start,
        })
    }
}
