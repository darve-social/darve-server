mod helpers;

use crate::helpers::create_fake_login_test_user;
use darve_server::entities::user_auth::local_user_entity::LocalUser;
use darve_server::interfaces::repositories::tags::TagsRepositoryInterface;
use darve_server::middleware::utils::db_utils::Pagination;

test_with_server!(test_create_with_relate, |server, ctx_state, config| {
    let (_, user, _, _) = create_fake_login_test_user(&server).await;
    let entity = user.id.unwrap();
    let tags = vec!["rust".to_string(), "backend".to_string()];
    let result = ctx_state.db.tags.create_with_relate(tags, entity).await;

    assert!(result.is_ok());

    // Verify tags created
    let saved_tags = ctx_state.db.tags.get().await.unwrap();
    assert!(saved_tags.contains(&"rust".to_string()));
    assert!(saved_tags.contains(&"backend".to_string()));
});

test_with_server!(test_get_by_tag, |server, ctx_state, config| {
    let (_, user, _, _) = create_fake_login_test_user(&server).await;
    let entity = user.id.unwrap();
    let tags = vec!["frontend".to_string(), "javascript".to_string()];
    let _ = ctx_state
        .db
        .tags
        .create_with_relate(tags.clone(), entity.clone())
        .await
        .unwrap();

    // Test getting entities by tag
    let pagination = Pagination {
        count: 10,
        start: 0,
        order_by: None,
        order_dir: None,
    };
    let results: Vec<LocalUser> = ctx_state
        .db
        .tags
        .get_by_tag("frontend", pagination)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let result = results.first().unwrap();
    assert_eq!(result.id.as_ref().unwrap().to_raw(), entity.to_raw());
});

test_with_server!(test_get_tags_empty, |_server, ctx_state, config| {
    let saved_tags = ctx_state.db.tags.get().await.unwrap();
    assert_eq!(saved_tags.len(), 0);
});
