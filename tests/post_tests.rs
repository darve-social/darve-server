mod helpers;

use crate::helpers::create_fake_login_test_user;
use crate::helpers::post_helpers::build_fake_post;
use crate::helpers::post_helpers::create_fake_post_with_large_file;
use crate::helpers::post_helpers::create_post;
use axum_test::multipart::MultipartForm;
use darve_server::entities::community::community_entity::CommunityDbService;
use darve_server::entities::community::discussion_entity::Discussion;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::entities::community::post_entity::Post;
use darve_server::entities::community::post_entity::PostDbService;
use darve_server::entities::community::post_entity::PostType;
use darve_server::entities::tag::SystemTags;
use darve_server::interfaces::repositories::tags::TagsRepositoryInterface;
use darve_server::middleware::utils::db_utils::Pagination;
use darve_server::middleware::utils::string_utils::get_string_thing;
use darve_server::middleware::{self};
use darve_server::models::view::discussion_user::DiscussionUserView;
use darve_server::models::view::post::PostView;
use darve_server::routes::posts::GetPostsQuery;
use darve_server::services::discussion_service::CreateDiscussion;
use fake::faker;
use fake::Fake;
use helpers::post_helpers::get_posts;
use helpers::post_helpers::{create_fake_post, create_fake_post_with_file};
use middleware::ctx::Ctx;

test_with_server!(create_post_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 4);
});

test_with_server!(
    create_post_with_the_same_name,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_ident = user.id.as_ref().unwrap().to_raw();

        let result =
            DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());

        let title = "TEST_TEST";
        let data = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");

        let response = helpers::post_helpers::create_post(server, &result, data).await;

        response.assert_status_success();

        let data_1 = MultipartForm::new()
            .add_text("title", title)
            .add_text("content", "content");
        let response_1 = helpers::post_helpers::create_post(server, &result, data_1).await;

        response_1.assert_status_success();
    }
);

test_with_server!(create_post_with_file_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let result = DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap());
    let _ = create_fake_post_with_large_file(server, &ctx_state, &result).await;
    let _ = create_fake_post_with_file(server, &ctx_state, &result).await;

    let posts = server
        .get(&format!("/api/discussions/{}/posts", result.to_raw()))
        .await
        .json::<Vec<PostView>>();

    let post = posts.first().unwrap();
    assert_eq!(post.media_links.as_ref().unwrap().len(), 1);
    assert!(post.media_links.as_ref().unwrap()[0].contains("file_example_PNG_1MB.png"));
});

test_with_server!(get_latest, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let user_ident = user.id.as_ref().unwrap().to_raw();
    let ctx = Ctx::new(Ok(user_ident.clone()), false);
    let user_thing_id = get_string_thing(user_ident).unwrap();

    let default_discussion = DiscussionDbService::get_profile_discussion_id(&user_thing_id);
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_disc(
        &user.id.as_ref().unwrap().id.to_raw(),
        &default_discussion.id.to_raw(),
        Some(PostType::Public),
        Pagination {
            order_by: None,
            order_dir: None,
            count: 2,
            start: 0,
        },
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);

    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_disc(
        &user.id.as_ref().unwrap().id.to_raw(),
        &default_discussion.id.to_raw(),
        Some(PostType::Public),
        Pagination {
            order_by: None,
            order_dir: None,
            count: 3,
            start: 0,
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 3);
    let result = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_disc(
        &user.id.as_ref().unwrap().id.to_raw(),
        &default_discussion.id.to_raw(),
        Some(PostType::Public),
        Pagination {
            order_by: None,
            order_dir: None,
            count: 1,
            start: 0,
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1)
});

test_with_server!(create_post_with_tags, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let tags = vec![
        "tag0".to_string(),
        "tag1".to_string(),
        "tag2".to_string(),
        "tag3".to_string(),
        "tag4".to_string(),
        "tag5".to_string(),
        "tag6".to_string(),
    ];
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(Vec::from(&tags[0..5])),
    )
    .await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 2);

    let tags = ctx_state
        .db
        .tags
        .get(
            None,
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap();
    assert!(tags.contains(&tags[0]));
    assert!(tags.contains(&tags[1]));
    assert!(tags.contains(&tags[2]));
    assert!(tags.contains(&tags[3]));
    assert!(tags.contains(&tags[4]));
});

test_with_server!(filter_posts_by_tag, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
    let tags = vec!["tag".to_string(), "tag1".to_string()];

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(vec![tags[0].clone()]),
    )
    .await;
    let _ = create_fake_post(server, &default_discussion, None, Some(tags.clone())).await;
    let _ = create_fake_post(
        server,
        &default_discussion,
        None,
        Some(vec!["non_of_them".to_string()]),
    )
    .await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 4);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: tags[0].clone(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 2);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: tags[1].clone(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 1);

    let posts_res = get_posts(
        &server,
        GetPostsQuery {
            tag: "rust".to_string(),
            start: None,
            count: None,
            order_dir: None,
        },
    )
    .await;

    posts_res.assert_status_success();
    let posts = posts_res.json::<Vec<PostView>>();
    assert_eq!(posts.len(), 0);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();
    assert_eq!(posts.len(), 4);
});

test_with_server!(
    try_to_create_without_content_and_file,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());
        let data = MultipartForm::new().add_text("title", "Hello");
        let response = create_post(server, &default_discussion, data).await;

        response.assert_status_failure();
        assert!(response.text().contains("Empty content and missing file"))
    }
);

test_with_server!(create_post_idea_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let data = MultipartForm::new()
        .add_text("title", "Hello")
        .add_text("is_idea", true)
        .add_text("content", "Sadasdas");

    let res = create_post(server, &default_discussion, data).await;
    res.assert_status_ok();

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts?filter_by_type=Idea",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 1);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts?filter_by_type=Public",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 0);
});

test_with_server!(
    try_to_create_post_idea_in_private_disc,
    |server, ctx_state, config| {
        let (server, user, _, token) = create_fake_login_test_user(&server).await;

        let comm_id = format!("community:{}", user.id.as_ref().unwrap().id.to_string());
        let create_response = server
            .post("/api/discussions")
            .add_header("Cookie", format!("jwt={}", token))
            .json(&CreateDiscussion {
                community_id: comm_id.clone(),
                title: "The Discussion".to_string(),
                image_uri: None,
                chat_user_ids: vec![user.id.as_ref().unwrap().to_raw()].into(),
                private_discussion_users_final: false,
            })
            .add_header("Accept", "application/json")
            .await;

        create_response.assert_status_success();
        let disc = create_response.json::<Discussion>();
        let data = MultipartForm::new()
            .add_text("title", "Hello")
            .add_text("is_idea", true)
            .add_text("content", "Sadasdas");

        let res = create_post(server, &disc.id, data).await;
        res.assert_status_forbidden();
    }
);

test_with_server!(
    try_to_create_post_idea_by_other_user,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let user_disc_id =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let (server, _, _, _) = create_fake_login_test_user(&server).await;

        let data = MultipartForm::new()
            .add_text("title", "Hello")
            .add_text("is_idea", true)
            .add_text("content", "Sadasdas");

        let res = create_post(server, &user_disc_id, data).await;
        res.assert_status_forbidden();
    }
);

test_with_server!(get_posts_by_filter, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;

    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;
    let _ = create_fake_post(server, &default_discussion, None, None).await;

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let _ = create_post(server, &default_discussion, data).await;

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let _ = create_post(server, &default_discussion, data).await;

    let data = MultipartForm::new()
        .add_text("title", faker::name::en::Name().fake::<String>())
        .add_text("is_idea", true)
        .add_text(
            "content",
            faker::lorem::en::Sentence(7..20).fake::<String>(),
        );

    let _ = create_post(server, &default_discussion, data).await;

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 7);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts?filter_by_type=Public",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 4);

    let posts = server
        .get(&format!(
            "/api/discussions/{}/posts?filter_by_type=Idea",
            default_discussion.to_raw()
        ))
        .await
        .json::<Vec<PostView>>();

    assert_eq!(posts.len(), 3);
});

test_with_server!(get_latest_posts, |server, state, config| {
    let (server, user2, _, token2) = create_fake_login_test_user(&server).await;
    let (server, user1, _, token1) = create_fake_login_test_user(&server).await;
    let (server, user, _, token) = create_fake_login_test_user(&server).await;

    let comm_id = CommunityDbService::get_profile_community_id(&user.id.as_ref().unwrap());
    let disc = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", token))
        .json(&CreateDiscussion {
            community_id: comm_id.to_raw(),
            title: "The Discussion".to_string(),
            image_uri: None,
            chat_user_ids: vec![
                user1.id.as_ref().unwrap().to_raw(),
                user2.id.as_ref().unwrap().to_raw(),
            ]
            .into(),
            private_discussion_users_final: false,
        })
        .add_header("Accept", "application/json")
        .await
        .json::<Discussion>();
    let disc_id = disc.id;
    let _ = create_fake_post(server, &disc_id, None, None).await;
    let _ = create_fake_post(server, &disc_id, None, None).await;
    let _ = create_fake_post(server, &disc_id, None, None).await;
    let post = create_fake_post(server, &disc_id, None, None).await;

    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id.to_raw(), post.id);

    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id.to_raw(), post.id);
    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id.to_raw(), post.id);

    let data = MultipartForm::new()
        .add_text("title", "Hello")
        .add_text("content", "content")
        .add_text("users", user.id.as_ref().unwrap().to_raw())
        .add_text("users", user1.id.as_ref().unwrap().to_raw());

    let private_post = create_post(server, &disc_id, data).await.json::<Post>();

    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id, *private_post.id.as_ref().unwrap());

    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token1))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id, *private_post.id.as_ref().unwrap());
    let latest_posts = server
        .get("/api/users/current/latest_posts")
        .add_header("Cookie", format!("jwt={}", token2))
        .await;

    latest_posts.assert_status_success();

    let posts = latest_posts.json::<Vec<DiscussionUserView>>();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].latest_post.id.to_raw(), post.id);
});

test_with_server!(get_post_by_id_test, |server, ctx_state, config| {
    let (server, user, _, _) = create_fake_login_test_user(&server).await;
    let default_discussion =
        DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

    // Create a test post
    let created_post = create_fake_post(server, &default_discussion, None, None).await;

    // Test successful retrieval
    let response = server.get(&format!("/api/posts/{}", created_post.id)).await;

    response.assert_status_ok();
    let post_view = response.json::<PostView>();
    assert_eq!(post_view.id.to_raw(), created_post.id);
    assert_eq!(post_view.created_by.id, user.id.as_ref().unwrap().clone());
    assert!(!post_view.title.is_empty());
});

test_with_server!(
    get_post_by_id_not_found_test,
    |server, ctx_state, config| {
        let (server, _, _, _) = create_fake_login_test_user(&server).await;

        let fake_post_id = "post:nonexistent";
        let response = server.get(&format!("/api/posts/{}", fake_post_id)).await;

        response.assert_status_not_found();
    }
);

test_with_server!(
    try_to_create_post_with_system_tags,
    |server, ctx_state, config| {
        let (server, user, _, _) = create_fake_login_test_user(&server).await;
        let default_discussion =
            DiscussionDbService::get_profile_discussion_id(&user.id.as_ref().unwrap());

        let _ = create_fake_post(server, &default_discussion, None, None).await;
        let tags = vec![
            SystemTags::Delivery.as_str().to_string(),
            "tag6".to_string(),
        ];
        let post_data = build_fake_post(None, Some(tags));
        let res = create_post(server, &default_discussion, post_data).await;
        res.assert_status_failure();
        assert!(res.text().contains(SystemTags::Delivery.as_str()))
    }
);
