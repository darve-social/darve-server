mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::entities::community::discussion_entity::DiscussionDbService;
use darve_server::entities::community::post_entity::Post;
use darve_server::interfaces::repositories::tags::TagsRepositoryInterface;
use darve_server::middleware::utils::db_utils::Pagination;
use darve_server::middleware::utils::string_utils::get_str_thing;
use helpers::post_helpers::create_fake_post;

test_with_server!(test_create_with_relate, |server, ctx_state, config| {
    let (_, user, _, token) = create_fake_login_test_user(&server).await;
    let entity = create_fake_post(
        &server,
        &DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap()),
        None,
        None,
        &token,
    )
    .await;
    let tags = vec!["rust".to_string(), "backend".to_string()];
    let result = ctx_state
        .db
        .tags
        .create_with_relate(tags.clone(), get_str_thing(&entity.id).unwrap())
        .await;
    assert!(result.is_ok());

    // Verify tags created
    let saved_tags = ctx_state
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
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect::<Vec<String>>();
    assert!(saved_tags.contains(&"rust".to_string()));
    assert!(saved_tags.contains(&"backend".to_string()));
});

test_with_server!(test_get_by_tag, |server, ctx_state, config| {
    let (_, user, _, token) = create_fake_login_test_user(&server).await;
    let entity = create_fake_post(
        &server,
        &DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap()),
        None,
        None,
        &token,
    )
    .await;
    let tags = vec!["frontend".to_string(), "javascript".to_string()];
    let _ = ctx_state
        .db
        .tags
        .create_with_relate(tags.clone(), get_str_thing(&entity.id).unwrap())
        .await
        .unwrap();

    // Test getting entities by tag
    let pagination = Pagination {
        count: 10,
        start: 0,
        order_by: None,
        order_dir: None,
    };
    let results: Vec<Post> = ctx_state
        .db
        .tags
        .get_by_tag("frontend", pagination)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let result = results.first().unwrap();
    assert_eq!(result.id.as_ref().unwrap().to_raw(), entity.id);
});

test_with_server!(test_get_tags_empty, |_server, ctx_state, config| {
    let saved_tags = ctx_state
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
    assert_eq!(saved_tags.len(), 0);
});

test_with_server!(filter_tags_by_start_with, |server, ctx_state, config| {
    let (_, user, _, token) = create_fake_login_test_user(&server).await;
    let entity = create_fake_post(
        &server,
        &DiscussionDbService::get_profile_discussion_id(user.id.as_ref().unwrap()),
        None,
        None,
        &token,
    )
    .await;
    let tags = vec![
        "rust".to_string(),
        "backend".to_string(),
        "java".to_string(),
        "javascript".to_string(),
    ];
    let result = ctx_state
        .db
        .tags
        .create_with_relate(tags.clone(), get_str_thing(&entity.id).unwrap())
        .await;

    assert!(result.is_ok());

    let saved_tags = ctx_state
        .db
        .tags
        .get(
            Some("ja".to_string()),
            Pagination {
                order_by: None,
                order_dir: None,
                count: 10,
                start: 0,
            },
        )
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect::<Vec<String>>();
    assert!(saved_tags.contains(&"java".to_string()));
    assert!(saved_tags.contains(&"javascript".to_string()));

    let saved_tags = ctx_state
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
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect::<Vec<String>>();
    assert_eq!(saved_tags[3], "rust".to_string());
    assert_eq!(saved_tags[2], "javascript".to_string());
    assert_eq!(saved_tags[1], "java".to_string());
    assert_eq!(saved_tags[0], "backend".to_string());
});
