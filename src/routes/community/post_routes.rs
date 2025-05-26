use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use axum_htmx::HX_REDIRECT;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde::{Deserialize, Serialize};
use std::path::Path as FPath;
use surrealdb::sql::Thing;
use validator::Validate;

use access_right_entity::AccessRightDbService;
use authorization_entity::{Authorization, AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_OWNER};
use community_routes::DiscussionNotificationEvent;
use discussion_entity::DiscussionDbService;
use discussion_notification_entity::{DiscussionNotification, DiscussionNotificationDbService};
use discussion_routes::{is_user_chat_discussion, DiscussionPostView, DiscussionView};
use discussion_topic_routes::DiscussionTopicView;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector, NO_SUCH_THING};
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use post_entity::{Post, PostDbService};
use post_stream_entity::PostStreamDbService;
use reply_routes::PostReplyView;
use tempfile::NamedTempFile;
use utils::template_utils::ProfileFormPage;

use crate::entities::community::{
    discussion_entity, discussion_notification_entity, post_entity, post_stream_entity,
};
use crate::entities::user_auth::{access_right_entity, authorization_entity, local_user_entity};
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::services::notification_service::NotificationService;
use crate::services::post_service::PostService;
use crate::{middleware, utils};

use super::discussion_routes::{DiscussionLatestPostCreatedBy, DiscussionLatestPostView};
use super::{community_routes, discussion_routes, discussion_topic_routes, reply_routes};

pub const UPLOADS_URL_BASE: &str = "/media";
pub fn routes(state: CtxState) -> Router {
    let view_routes = Router::new().route("/discussion/:discussion_id/post", get(create_form));
    // .route("/discussion/:discussion_id/post/:title_uri", get(get_post));

    let max_bytes_val = (1024 * 1024 * state.upload_max_size_mb) as usize;
    Router::new()
        .merge(view_routes)
        .route("/api/posts", get(get_posts))
        .route("/api/posts/:post_id/like", post(like))
        .route("/api/posts/:post_id/unlike", delete(unlike))
        .route(
            "/api/discussion/:discussion_id/post",
            post(create_post_entity_route),
        )
        .nest_service(UPLOADS_URL_BASE, state.uploads_serve_dir.clone())
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_bytes_val))
}

#[derive(Deserialize)]
struct PostDiscussionCommunityOwnerView {
    created_by_profile_profile_discussion: Option<Thing>,
    belongs_to: Thing,
    community_uri: String,
    username: String,
}

impl ViewFieldSelector for PostDiscussionCommunityOwnerView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        // belongs_to == discussion
        // belongs_to.belongs_to == community
        "belongs_to, belongs_to.belongs_to.created_by.community.default_discussion as created_by_profile_profile_discussion, belongs_to.belongs_to.name_uri as community_uri, belongs_to.belongs_to.created_by.username as username".to_string()
    }
}

#[derive(Validate, TryFromMultipart)]
pub struct PostInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
    pub topic_id: String,
    #[validate(length(max = 5, message = "Max 5 tags"))]
    pub tags: Vec<String>,
    #[form_data(limit = "unlimited")]
    pub file_1: Option<FieldData<NamedTempFile>>,
}

#[derive(Template, Serialize)]
#[template(path = "nera2/post.html")]
pub struct PostPageTemplate {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    title_uri: String,
    discussion_id: String,
    discussion_topic: Option<Thing>,
    title: String,
    content: String,
    replies: Vec<PostReplyView>,
}

#[derive(Template, Serialize)]
#[template(path = "nera2/post_form.html")]
struct PostFormTemplate {
    discussion_id: String,
    title: String,
    content: String,
    topics: Vec<DiscussionTopicView>,
}

impl From<Post> for PostPageTemplate {
    fn from(value: Post) -> Self {
        PostPageTemplate {
            theme_name: "dark".to_string(),
            window_title: "winn".to_string(),
            nav_top_title: "npost".to_string(),
            header_title: "header".to_string(),
            footer_text: "foo".to_string(),
            title: value.title,
            content: value.content,
            title_uri: value.r_title_uri.unwrap(),
            discussion_id: value.belongs_to.to_raw(),
            replies: vec![],
            discussion_topic: value.discussion_topic,
            // replies: if let Some(posts) = value.r_replies { posts } else { vec![] },
        }
    }
}

// commenting this out as it not used anywhere - @anukulpandey
// async fn get_post(
//     State(CtxState { _db, .. }): State<CtxState>,
//     ctx: Ctx,
//     Path(disc_id_title_uri): Path<(String, String)>,
// ) -> CtxResult<PostPageTemplate> {
//     println!("->> {:<12} - get post", "HANDLER");

//     let comm_db = DiscussionDbService {
//         db: &_db,
//         ctx: &ctx,
//     };
//     let discussion = comm_db
//         .must_exist(IdentIdName::Id(get_string_thing(disc_id_title_uri.0)?))
//         .await?;

//     let ident = IdentIdName::ColumnIdentAnd(vec![
//         IdentIdName::ColumnIdent {
//             column: "belongs_to".to_string(),
//             val: discussion.to_raw(),
//             rec: true,
//         },
//         IdentIdName::ColumnIdent {
//             column: "r_title_uri".to_string(),
//             val: disc_id_title_uri.1,
//             rec: false,
//         },
//     ]);
//     let mut post = PostDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .get(ident)
//     .await?;
//     let post_replies = ReplyDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .get_by_post_desc_view::<PostReplyView>(post.id.clone().unwrap(), 0, 120)
//     .await?;

//     let mut post_page: PostPageTemplate = post.into();
//     post_page.replies = post_replies;
//     Ok(post_page)
// }

#[derive(Debug, Deserialize)]
pub struct GetPostsQuery {
    pub tag: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPostsResponse {
    pub posts: Vec<Post>,
}

async fn get_posts(
    Query(query): Query<GetPostsQuery>,
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Json<GetPostsResponse>> {
    let post_db_service = PostDbService {
        ctx: &ctx,
        db: &_db,
    };
    let pagination = Pagination {
        order_by: Some("id".to_string()),
        order_dir: query.order_dir,
        count: query.count.unwrap_or(100) as i8,
        start: query.start.unwrap_or_default() as i32,
    };
    let posts = post_db_service.get_by_tag(query.tag, pagination).await?;
    Ok(Json(GetPostsResponse { posts }))
}

async fn create_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
) -> CtxResult<ProfileFormPage> {
    let user_id = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let disc_id = get_string_thing(discussion_id.clone())?;

    let required_comm_auth = Authorization {
        authorize_record_id: disc_id.clone(),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 99,
    };
    AccessRightDbService {
        db: &_db,
        ctx: &ctx,
    }
    .is_authorized(&user_id, &required_comm_auth)
    .await?;

    let dis_template = DiscussionDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_view::<DiscussionView>(IdentIdName::Id(disc_id))
    .await?;

    let topics: Vec<DiscussionTopicView> = dis_template.topics.unwrap_or(vec![]);

    Ok(ProfileFormPage::new(
        Box::new(PostFormTemplate {
            discussion_id,
            title: "".to_string(),
            content: "".to_string(),
            topics,
        }),
        None,
        None,
        None,
    ))
}

pub async fn create_post_entity_route(
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    State(ctx_state): State<CtxState>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Response> {
    input_value.validate()?;

    let user = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;
    let user_id = user.id.expect("user exists");
    let disc_db = DiscussionDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let disc = disc_db
        .get(IdentIdName::Id(get_string_thing(discussion_id)?))
        .await?;

    let is_user_chat = is_user_chat_discussion(&ctx, &disc.chat_room_user_ids).unwrap_or(false);

    if !is_user_chat {
        let min_authorisation = Authorization {
            authorize_record_id: disc.id.clone().unwrap().clone(),
            authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(),
            authorize_height: 0,
        };
        AccessRightDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        }
        .is_authorized(&user_id, &min_authorisation)
        .await?;
    }

    let post_db_service = PostDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };

    let new_post_id = PostDbService::get_new_post_thing();
    let mut media_links = vec![];
    // try saving file first so post is not created in case it fails
    if let Some(files) = input_value.file_1 {
        let file_name = files.metadata.file_name.unwrap();
        let ext = file_name.split(".").last().ok_or(AppError::Generic {
            description: "File has no extension".to_string(),
        })?;

        let file_name = format!("pid_{}-file_1.{ext}", new_post_id.to_raw());
        let path = FPath::new(&ctx_state.uploads_dir).join(file_name.as_str());
        let saved = files.contents.persist(path.clone());
        if saved.is_ok() {
            media_links.push(format!("{UPLOADS_URL_BASE}/{file_name}"));
        } else {
            return Err(ctx.to_ctx_error(AppError::Generic {
                description: saved.err().expect("is error").to_string(),
            }));
        }
    };

    let topic_val: Option<Thing> = if input_value.topic_id.trim().len() > 0 {
        get_string_thing(input_value.topic_id).ok()
    } else {
        None
    };
    let post = post_db_service
        .create_update(Post {
            id: Some(new_post_id),
            belongs_to: disc.id.clone().unwrap(),
            discussion_topic: topic_val.clone(),
            title: input_value.title,
            r_title_uri: None,
            content: input_value.content,
            media_links: if media_links.is_empty() {
                None
            } else {
                Some(media_links)
            },
            metadata: None,
            r_created: None,
            created_by: user_id.clone(),
            r_updated: None,
            r_replies: None,
            likes_nr: 0,
            replies_nr: 0,
            tags: if input_value.tags.is_empty() {
                None
            } else {
                Some(input_value.tags)
            },
        })
        .await?;

    // set latest post
    disc_db
        .set_latest_post_id(disc.id.clone().unwrap(), post.id.clone().unwrap())
        .await?;

    let latest_post = DiscussionLatestPostView {
        id: post.belongs_to,
        created_by: DiscussionLatestPostCreatedBy {
            id: user_id.clone(),
            username: user.username,
            full_name: user.full_name,
            image_uri: user.image_uri,
        },
        title: post.title,
        content: post.content,
        media_links: post.media_links,
        r_created: post.r_created,
    };

    let n_service = NotificationService::new(&ctx_state._db, &ctx, &ctx_state.event_sender);
    let content = serde_json::to_string(&latest_post).unwrap();
    if is_user_chat {
        n_service
            .on_chat_message(
                &user_id,
                &disc.chat_room_user_ids.clone().unwrap(),
                &content,
            )
            .await?;
    } else {
        n_service.on_community_post(&user_id, &content).await?;

        PostStreamDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        }
        .to_user_follower_streams(post.created_by.clone(), &post.id.clone().expect("has id"))
        .await?;
    }

    let post_comm_view = post_db_service
        .get_view::<DiscussionPostView>(IdentIdName::Id(post.id.clone().unwrap()))
        .await?;
    let notif_db_ser = DiscussionNotificationDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let post_json = serde_json::to_string(&post_comm_view).map_err(|_| {
        ctx.to_ctx_error(AppError::Generic {
            description: "Post to json error for notification event".to_string(),
        })
    })?;

    let event_type: String = DiscussionNotificationEvent::DiscussionPostAdded {
        discussion_id: NO_SUCH_THING.clone(),
        topic_id: None,
        post_id: NO_SUCH_THING.clone(),
    }
    .to_string();
    let event = DiscussionNotificationEvent::try_from_post(event_type.as_str(), &post_comm_view)?;
    notif_db_ser
        .create(DiscussionNotification {
            id: None,
            event,
            content: post_json,
            r_created: None,
        })
        .await?;

    let res = CreatedResponse {
        success: true,
        id: post.id.clone().unwrap().to_raw(),
        uri: post.r_title_uri,
    };
    // let created_uri = &res.uri.clone().unwrap();
    let mut res = ctx.to_htmx_or_json::<CreatedResponse>(res)?.into_response();
    let redirect = get_post_home_uri(&ctx_state, &ctx, post.id.unwrap()).await?;
    res.headers_mut().append(
        HX_REDIRECT,
        HeaderValue::from_str(redirect.as_str()).expect("header value"),
    );
    Ok(res)
}

async fn get_post_home_uri(ctx_state: &CtxState, ctx: &Ctx, post_id: Thing) -> CtxResult<String> {
    let owner_view = PostDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_view::<PostDiscussionCommunityOwnerView>(IdentIdName::Id(post_id))
    .await?;
    // belongs_to = discussion
    if owner_view.created_by_profile_profile_discussion == Some(owner_view.belongs_to) {
        Ok(format!("/u/{}", owner_view.username))
    } else {
        Ok(format!("/community/{}", owner_view.community_uri))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostLikeResponse {
    pub likes_count: u32,
}

async fn like(
    ctx: Ctx,
    Path(post_id): Path<String>,
    State(ctx_state): State<CtxState>,
) -> CtxResult<Json<PostLikeResponse>> {
    let user = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    let post_thing = get_string_thing(post_id)?;

    let count = PostService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .like(&post_thing, &user)
    .await?;

    let user_id = user.id.unwrap();

    let n_service = NotificationService::new(&ctx_state._db, &ctx, &ctx_state.event_sender);
    n_service
        .on_like(&user_id, vec![user_id.clone()], post_thing)
        .await?;

    Ok(Json(PostLikeResponse { likes_count: count }))
}

async fn unlike(
    ctx: Ctx,
    Path(post_id): Path<String>,
    State(ctx_state): State<CtxState>,
) -> CtxResult<Json<PostLikeResponse>> {
    let user = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    let count = PostService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .unlike(post_id, &user)
    .await?;

    Ok(Json(PostLikeResponse { likes_count: count }))
}
