use std::{convert::Infallible, sync::Arc};

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    middleware::{
        ctx::Ctx,
        error::CtxResult,
        mw_ctx::{AppEventType, CtxState},
    },
};
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::get,
    Router,
};
use futures::Stream;
use serde_json::json;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/api/events/sse", get(get_events_sse))
}

async fn get_events_sse(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let user_id = user.to_raw();

    let rx = state.event_sender.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |msg| match msg {
        Err(_) => None,
        Ok(msg) => match msg.event {
            AppEventType::UserNotificationEvent(..) if msg.receivers.contains(&user_id) => {
                Some(Ok(Event::default().data(json!(msg).to_string())))
            }
            _ => None,
        },
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
