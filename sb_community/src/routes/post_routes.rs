use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use axum_htmx::HX_REDIRECT;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde::{Deserialize, Serialize};
use std::path::Path as FPath;
use surrealdb::sql::Thing;
use validator::Validate;

use sb_user_auth::entity::authorization_entity::{Authorization, AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_OWNER};
use crate::entity::discussion_entitiy::DiscussionDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::notification_entitiy::{Notification, NotificationDbService};
use crate::entity::post_entitiy::{Post, PostDbService};
use crate::entity::reply_entitiy::ReplyDbService;
use crate::routes::discussion_routes::{DiscussionPostView, DiscussionView};
use crate::routes::discussion_topic_routes::DiscussionTopicView;
use crate::routes::reply_routes::PostReplyView;
use sb_user_auth::utils::template_utils::ProfileFormPage;
use sb_user_auth::utils::askama_filter_util::filters;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::request_utils::CreatedResponse;
use tempfile::NamedTempFile;
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use crate::routes::community_routes::{CommunityNotificationEvent, PostNotificationEventIdent};

pub const UPLOADS_URL_BASE:&str = "/media";
pub fn routes(state: CtxState) -> Router {
    let view_routes = Router::new()
        .route("/discussion/:discussion_id/post", get(create_form));
        // .route("/discussion/:discussion_id/post/:title_uri", get(get_post));

    Router::new()
        .merge(view_routes)
        .route("/api/discussion/:discussion_id/post", post(create_entity))
        .nest_service(UPLOADS_URL_BASE, tower_http::services::ServeDir::new(state.uploads_dir.clone()))
        .layer(DefaultBodyLimit::max(1024*1024*15))
        .with_state(state)
}

#[derive(Deserialize)]
struct PostDiscussionCommunityOwnerView {
    created_by_profile_main_discussion: Option<Thing>,
    belongs_to: Thing,
    community_uri: String,
    username: String,
}

impl ViewFieldSelector for PostDiscussionCommunityOwnerView {
    fn get_select_query_fields(ident: &IdentIdName) -> String {
        // belongs_to == discussion
        // belongs_to.belongs_to == community
        "belongs_to, belongs_to.belongs_to.created_by.community.main_discussion as created_by_profile_main_discussion, belongs_to.belongs_to.name_uri as community_uri, belongs_to.belongs_to.created_by.username as username".to_string()
    }
}

#[derive( Validate, TryFromMultipart)]
pub struct PostInput {
    // #[validate(length(min = 5, message = "Min 5 characters"))]
    // pub discussion_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
    pub topic_id: String,
    #[form_data(limit = "3MiB")]
    pub file_1: Option<FieldData<NamedTempFile>>,
    #[form_data(limit = "3MiB")]
    pub file_2: Option<FieldData<NamedTempFile>>,
    #[form_data(limit = "3MiB")]
    pub file_3: Option<FieldData<NamedTempFile>>,
    #[form_data(limit = "3MiB")]
    pub file_4: Option<FieldData<NamedTempFile>>,
    #[form_data(limit = "3MiB")]
    pub file_5: Option<FieldData<NamedTempFile>>,
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

async fn get_post(State(CtxState { _db, .. }): State<CtxState>,
                  ctx: Ctx,
                  Path(disc_id__title_uri): Path<(String, String)>,
) -> CtxResult<PostPageTemplate> {
    println!("->> {:<12} - get post", "HANDLER");

    let comm_db = DiscussionDbService { db: &_db, ctx: &ctx };
    let discussion = comm_db.must_exist(IdentIdName::Id(disc_id__title_uri.0)).await?;

    let ident = IdentIdName::ColumnIdentAnd(vec![
        IdentIdName::ColumnIdent { column: "belongs_to".to_string(), val: discussion.to_raw(), rec: true},
        IdentIdName::ColumnIdent { column: "r_title_uri".to_string(), val: disc_id__title_uri.1, rec: false},
    ]);
    let mut post = PostDbService { db: &_db, ctx: &ctx }.get(ident).await?;
    let post_replies = ReplyDbService { db: &_db, ctx: &ctx }.get_by_post_desc_view::<PostReplyView>(post.id.clone().unwrap(), 0, 120).await?;

    let mut postPage: PostPageTemplate = post.into();
    postPage.replies = post_replies;
    Ok(postPage)
}

async fn create_form(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
) -> CtxResult<ProfileFormPage> {
    let user_id = LocalUserDbService{ db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let disc_id = Thing::try_from(discussion_id.clone()).map_err(|e| ctx.to_api_error(AppError::Generic { description: "error into discussion Thing".to_string() }))?;

    let required_comm_auth = Authorization { authorize_record_id: disc_id, authorize_activity: AUTH_ACTIVITY_OWNER.to_string(), authorize_height: 99 };
    AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&user_id, &required_comm_auth).await?;

    let dis_template = DiscussionDbService { db: &_db, ctx: &ctx }.get_view::<DiscussionView>(IdentIdName::Id(discussion_id.clone())).await?;

    let topics: Vec<DiscussionTopicView> = dis_template.topics.unwrap_or(vec![]);

    Ok(ProfileFormPage::new(Box::new(PostFormTemplate {
        discussion_id,
        title: "".to_string(),
        content: "".to_string(),
        topics
    }), None, None))

}

async fn create_entity(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       Path(discussion_id): Path<String>,
                       State(ctx_state): State<CtxState>,
                       TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Response> {
    println!("->> {:<12} - create_post ", "HANDLER");
    let created_by = LocalUserDbService{ db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let disc_db = DiscussionDbService { db: &_db, ctx: &ctx };
    let disc_id = disc_db.must_exist(IdentIdName::Id(discussion_id) ).await?;

    let min_authorisation = Authorization{
        authorize_record_id: disc_id.clone(),
        authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(),
        authorize_height: 0,
    };

    AccessRightDbService { db: &_db, ctx: &ctx }.is_authorized(&created_by, &min_authorisation).await?;

    let post_db_service = PostDbService { db: &_db, ctx: &ctx };
    let topic_val: Option<Thing> = if input_value.topic_id.trim().len() > 0 {
        Thing::try_from(input_value.topic_id).ok()
    } else { None };

    let post = post_db_service
        .create(Post { id: None, belongs_to: disc_id.clone(), discussion_topic: topic_val.clone(), title: input_value.title, r_title_uri: None, content: input_value.content, media_links: None, r_created: None, created_by, r_updated: None, r_replies: None, likes_nr: 0, replies_nr: 0 })
        .await?;

    if let Some(files) = input_value.file_1 {
        let file_name = files.metadata.file_name.unwrap();
        let ext = file_name.split(".").last().ok_or(AppError::Generic {description:"File has no extension".to_string()})?;

        let file_name = format!("pid_{}-file_1.{ext}", post.id.clone().unwrap().id.to_raw());
        let path = FPath::new(&ctx_state.uploads_dir).join(file_name.as_str());
        let saved= files.contents.persist(path.clone());
        if saved.is_ok(){
            post_db_service.set_media_url(post.id.clone().unwrap(), format!("{UPLOADS_URL_BASE}/{file_name}").as_str()).await?;
        }
    }

    let post_comm_view = post_db_service.get_view::<DiscussionPostView>(IdentIdName::Id(post.id.clone().unwrap().to_raw())).await?;
    let notif_db_ser = NotificationDbService { db: &_db, ctx: &ctx };
    let post_json = serde_json::to_string(&post_comm_view).map_err(|e1| ctx.to_api_error(AppError::Generic {description:"Post to json error for notification event".to_string()}))?;

    let event_ident = String::try_from(&PostNotificationEventIdent::from(&post_comm_view)).ok();
    notif_db_ser.create(
        Notification { id: None, event_ident, event: CommunityNotificationEvent::Discussion_PostAdded.to_string(), content: post_json, r_created: None }
    ).await?;

    let res = CreatedResponse { success: true, id: post.id.clone().unwrap().to_raw(), uri: post.r_title_uri };
    // let created_uri = &res.uri.clone().unwrap();
    let mut res = ctx.to_htmx_or_json::<CreatedResponse>(res)
        .into_response();
    let redirect= get_post_home_uri(&ctx_state, &ctx, post.id.unwrap()).await?;
    res.headers_mut().append(HX_REDIRECT, HeaderValue::from_str(redirect.as_str()).expect("header value"));
    Ok(res)
}

async fn get_post_home_uri(ctx_state: &CtxState, ctx: &Ctx, post_id: Thing) -> CtxResult<String> {
    let owner_view = PostDbService{db: &ctx_state._db, ctx: &ctx}.get_view::<PostDiscussionCommunityOwnerView>(IdentIdName::Id(post_id.to_raw())).await?;
    // belongs_to = discussion
    if owner_view.created_by_profile_main_discussion == Some(owner_view.belongs_to) {
        Ok(format!("/u/{}", owner_view.username))
    } else { Ok(format!("/community/{}", owner_view.community_uri)) }
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum_test::multipart::MultipartForm;
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::entity::community_entitiy::{Community, CommunityDbService};
    use crate::routes::community_routes::{get_community, CommunityInput};
    use crate::utils::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::extractor_utils::DiscussionParams;
    use sb_middleware::utils::request_utils::CreatedResponse;

    #[tokio::test]
    async fn create_post() {
        let (server, ctx_state) = create_test_server().await;
        let server = server.unwrap();
        let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

        let comm_name = "comm-naMMe1".to_lowercase();

        let create_response = server.post("/api/community").json(&CommunityInput { id: "".to_string(), create_custom_id: None, name_uri: comm_name.clone(), title: "The Community Test".to_string() }).await;
        let created = &create_response.json::<CreatedResponse>();

        let comm_id = Thing::try_from(created.id.clone()).unwrap();
        let comm_name = created.uri.clone().unwrap();
        &create_response.assert_status_success();

        let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);
        let community_db_service = CommunityDbService { db: &ctx_state._db, ctx: &ctx };
        let community:Community = community_db_service.db.select((&comm_id.tb, comm_id.id.to_raw())).await.unwrap().unwrap();
        assert_eq!(comm_name, community.name_uri.clone());
        let community_discussion_id = community.main_discussion.clone().unwrap();

        let post_name = "post title Name 1".to_string();
        let create_post = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", post_name.clone()).add_text("content", "contentttt").add_text("topic_id", "")).await;
        let created = create_post.json::<CreatedResponse>();
        &create_post.assert_status_success();
        assert_eq!(created.id.len() > 0, true);

        let postName2 = "post title Name 2?&$^%! <>end".to_string();
        let create_response2 = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", postName2.clone()).add_text("content", "contentttt222").add_text("topic_id", "")).await;

        let create_response4 = server.post(format!("/api/discussion/{community_discussion_id}/post").as_str()).multipart(MultipartForm::new().add_text("title", postName2.clone()).add_text("content", "contentttt444442").add_text("topic_id", "")).await;
        let created = &create_response.json::<CreatedResponse>();
        let created2 = &create_response2.json::<CreatedResponse>();

        &create_response2.assert_status_success();
        // can't have same title
        &create_response4.assert_status_bad_request();

        let comm_posts_response = server.get(format!("/api/discussion/{community_discussion_id}/post").as_str()).await;

        let comm_view = get_community(State(ctx_state), ctx, Path(comm_name), DiscussionParams{
            topic_id: None,
            start: None,
            count: None,
        }).await.expect("community page");
        let posts = comm_view.community_view.unwrap().main_discussion_view.unwrap().posts;
        assert_eq!(posts.len(), 2);
    }

}

