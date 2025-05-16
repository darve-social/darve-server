mod helpers;

use authorization_entity::{
    get_root_auth_rec_name, Authorization, AUTH_ACTIVITY_ADMIN, AUTH_ACTIVITY_EDITOR,
    AUTH_ACTIVITY_MEMBER, AUTH_ACTIVITY_OWNER, AUTH_REC_NAME_DISCUSSION, AUTH_REC_NAME_POST,
};
use darve_server::entities::user_auth::authorization_entity;
use helpers::{create_login_test_user, create_test_server};
use surrealdb::sql::Thing;

#[tokio::test]
async fn authorization_compare() {
    let root_rec = "community";

    let a1 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: "general-content".into(),
        authorize_height: 11,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: "general-content".into(),
        authorize_height: 17,
    };

    let a1_str: String = a1.into();
    let a2_str: String = a2.into();

    let a_1: Authorization = a1_str.try_into().unwrap();
    let a_2: Authorization = a2_str.try_into().unwrap();

    assert_eq!(a_1.authorize_record_id.tb, root_rec);
    assert_eq!(a_1.authorize_activity, "general-content");
    assert_eq!(a_1.authorize_height, 11);

    assert_eq!(a_2.authorize_record_id.tb, root_rec);
    assert_eq!(a_2.authorize_activity, "general-content");
    assert_eq!(a_2.authorize_height, 17);

    let a1 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_EDITOR.into(),
        authorize_height: 11,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_EDITOR.into(),
        authorize_height: 17,
    };

    assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

    let a1 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_EDITOR.into(),
        authorize_height: 17,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_EDITOR.into(),
        authorize_height: 17,
    };

    assert_eq!(a1.ge_equal_ident(&a2).unwrap(), true);

    let a1 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_EDITOR.into(),
        authorize_height: 11,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_MEMBER.into(),
        authorize_height: 17,
    };

    assert_eq!(a1.ge_equal_ident(&a2).unwrap(), true);

    let a1 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_MEMBER.into(),
        authorize_height: 112,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((root_rec, "123")),
        authorize_activity: AUTH_ACTIVITY_ADMIN.into(),
        authorize_height: 17,
    };

    assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

    let a1 = Authorization {
        authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")),
        authorize_activity: AUTH_ACTIVITY_MEMBER.into(),
        authorize_height: 112,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")),
        authorize_activity: AUTH_ACTIVITY_ADMIN.into(),
        authorize_height: 17,
    };

    assert_eq!(a1.ge_equal_ident(&a2).unwrap(), false);

    let a1 = Authorization {
        authorize_record_id: Thing::from((AUTH_REC_NAME_POST.into(), "123")),
        authorize_activity: AUTH_ACTIVITY_MEMBER.into(),
        authorize_height: 11,
    };
    let a2 = Authorization {
        authorize_record_id: Thing::from((AUTH_REC_NAME_DISCUSSION.into(), "123")),
        authorize_activity: AUTH_ACTIVITY_ADMIN.into(),
        authorize_height: 17,
    };
    println!("EXPECTED TEST ERR={:?}", a1.ge_equal_ident(&a2));
    assert_eq!(a1.ge_equal_ident(&a2).is_err(), true);

    let smaller_auth = Authorization {
        authorize_record_id: Thing::from(("discussion", "234")),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 98,
    };
    let higher_auth = Authorization {
        authorize_record_id: Thing::from(("discussion", "234")),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 100,
    };
    assert_eq!(smaller_auth.ge_equal_ident(&higher_auth).is_err(), false);
    assert_eq!(
        smaller_auth.clone().ge_equal_ident(&higher_auth).unwrap(),
        false
    );

    let compare = Authorization {
        authorize_record_id: Thing::from((
            AUTH_REC_NAME_DISCUSSION.to_string(),
            String::from("ggphl6f1ng8v1keac9jl"),
        )),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 11,
    };
    let compare_to = [
        Authorization {
            authorize_record_id: Thing::from((
                AUTH_REC_NAME_DISCUSSION.to_string(),
                String::from("ggphl6f1ng8v1keac9jl"),
            )),
            authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(),
            authorize_height: 99,
        },
        Authorization {
            authorize_record_id: Thing::from((
                AUTH_REC_NAME_DISCUSSION.to_string(),
                String::from("ggphl6f1ng8v1keac9jl"),
            )),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 10,
        },
    ];

    assert_eq!(
        compare
            .clone()
            .ge_equal_ident(&compare_to.clone()[0])
            .unwrap(),
        true
    );
    assert_eq!(
        compare
            .clone()
            .ge_equal_ident(&compare_to.clone()[1])
            .unwrap(),
        true
    );

    let pos = compare_to
        .into_iter()
        .position(|a| a.ge_equal_ident(&compare).unwrap());
    assert_eq!(pos, None);

    let compare = Authorization {
        authorize_record_id: Thing::from((
            get_root_auth_rec_name(),
            String::from("qlwk1gd4tjj60gj16dn3"),
        )),
        authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(),
        authorize_height: 22,
    };
    let compare_to = [
        Authorization {
            authorize_record_id: Thing::from((
                get_root_auth_rec_name(),
                String::from("mfoo20kla99blv4mcelp"),
            )),
            authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(),
            authorize_height: 99,
        },
        Authorization {
            authorize_record_id: Thing::from((
                get_root_auth_rec_name(),
                String::from("mfoo20kla99blv4mcelp"),
            )),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        },
        Authorization {
            authorize_record_id: Thing::from((
                "discussion".to_string(),
                String::from("ggphl6f1ng8v1keac9jl"),
            )),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        },
    ];

    let pos = compare_to
        .into_iter()
        .position(|a| compare.ge_equal_ident(&a).unwrap());
    assert_eq!(pos, Some(2));

    let compare = Authorization {
        authorize_record_id: Thing::from((
            get_root_auth_rec_name(),
            String::from("qlwk1gd4tjj60gj16dn3"),
        )),
        authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
        authorize_height: 22,
    };
    let compare_to = [
        Authorization {
            authorize_record_id: Thing::from((
                get_root_auth_rec_name(),
                String::from("mfoo20kla99blv4mcelp"),
            )),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        },
        Authorization {
            authorize_record_id: Thing::from((
                get_root_auth_rec_name(),
                String::from("mfoo20kla99blv4mcelp"),
            )),
            authorize_activity: AUTH_ACTIVITY_EDITOR.to_string(),
            authorize_height: 99,
        },
    ];

    let pos = compare_to
        .into_iter()
        .position(|a| compare.ge_equal_ident(&a).unwrap());
    assert_eq!(pos, Some(1));
}

#[tokio::test]
async fn get_auth_parent_record_id() {
    let (server, _) = create_test_server().await;
    let (_, _) = create_login_test_user(&server, "usnnnn".to_string()).await;
}

// TODO: Fix failing test
// #[tokio::test]
// async fn authorize_save() {
//     let (server, ctx_state) = create_test_server().await;
//     let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;

//     let ctx = Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false);
//     let create_response = server
//         .post("/api/community")
//         .json(&CommunityInput {
//             id: "".to_string(),
//             name_uri: "community-123".to_string(),
//             title: "The Community Test".to_string(),
//         })
//         .await;
//     let created = &create_response.json::<CreatedResponse>();
//     // dbg!(&created);
//     let comm_id = Thing::try_from(created.id.clone()).unwrap();
//     let _ = created.uri.clone().unwrap();
//     create_response.assert_status_success();

//     let community_db_service = CommunityDbService {
//         db: &ctx_state._db,
//         ctx: &ctx.clone(),
//     };
//     let community: Option<Community> = community_db_service
//         .db
//         .select((comm_id.clone().tb, comm_id.id.to_raw()))
//         .await
//         .unwrap();

//     let comm_discussion_id = community.unwrap().profile_discussion.unwrap();

//     let acc_right_service = AccessRightDbService {
//         db: &ctx_state._db,
//         ctx: &Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false),
//     };

//     // let root_auth_rec = get_root_auth_rec_name();
//     let auth_rec = comm_discussion_id;
//     let auth_rec2 = comm_id;

//     let authorization =
//         Authorization::new(auth_rec.clone(), AUTH_ACTIVITY_OWNER.to_string(), 10).unwrap();
//     let authorization_comm_owner =
//         Authorization::new(auth_rec2.clone(), AUTH_ACTIVITY_OWNER.to_string(), 10).unwrap();

//     let is_auth = acc_right_service
//         .is_authorized(
//             &Thing::try_from(user_ident.clone()).unwrap(),
//             &authorization_comm_owner,
//         )
//         .await;
//     assert_eq!(is_auth.is_ok(), true);

//     let auth = acc_right_service
//         .authorize(
//             Thing::try_from(user_ident.clone()).unwrap(),
//             authorization,
//             None,
//         )
//         .await;
//     assert_eq!(auth.is_ok(), true);
//     let authorization2 =
//         Authorization::new(auth_rec2.clone(), AUTH_ACTIVITY_VISITOR.to_string(), 101);
//     dbg!(&authorization2);
//     let auth = acc_right_service
//         .authorize(
//             Thing::try_from(user_ident.clone()).unwrap(),
//             authorization2.clone().unwrap(),
//             None,
//         )
//         .await;
//     dbg!(&auth);
//     assert_eq!(auth.is_ok(), true);

//     let mut authorizations = acc_right_service
//         .get_authorizations(&Thing::try_from(user_ident.clone()).unwrap())
//         .await
//         .unwrap();
//     let auth_iter = authorizations.iter_mut();
//     let is_authorized = acc_right_service
//         .is_authorized(
//             &Thing::try_from(user_ident.clone()).unwrap(),
//             &authorization2.clone().unwrap(),
//         )
//         .await;
//     assert_eq!(auth_iter.len(), 1);
//     assert_eq!(is_authorized.is_ok(), true);

//     /*let auth_1 = auth_iter.next().unwrap();
//     let a_right = AccessRightDbService { db: &ctx_state._db, ctx: &Ctx::new(Ok(user_ident.clone()), Uuid::new_v4(), false) }
//         .get(IdentIdName::ColumnIdent{rec:false, column:"authorization".to_string(), val: auth_1.to}).await.unwrap();
//     assert_eq!(a_right.authorization.authorize_activity.clone(), AUTH_ACTIVITY_OWNER.to_string());
//     assert_eq!(a_right.authorization.authorize_record_id.tb, auth_rec2.tb);*/
//     // TODO test auth on posts etc...
// }
