use crate::{
    access::base::{
        access_control, control::AccessControl, path::AccessPath, permission::Permission,
        resource::Resource, role::Role,
    },
    entities::{community::post_entity::PostType, user_auth::local_user_entity::LocalUser},
    models::view::access::PostAccessView,
};

impl AccessPath {
    pub fn from_post(
        post: &PostAccessView,
        user: Option<&LocalUser>,
        next: Option<Box<AccessPath>>,
    ) -> Self {
        let role = match user {
            Some(user) => post
                .users
                .iter()
                .find(|u| u.user == *user.id.as_ref().unwrap())
                .map_or(Role::Guest, |u| Role::from(u.role.as_str())),
            None => Role::Guest,
        };

        let access_path = Some(Box::new(AccessPath {
            name: match post.r#type {
                PostType::Private => Resource::PostPrivate,
                PostType::Public => Resource::PostPublic,
            },
            role,
            next,
        }));

        AccessPath::from_discussion(&post.discussion, user, access_path)
    }
}

pub struct PostAccess<'a> {
    post: &'a PostAccessView,
    access_control: &'static AccessControl,
}

impl<'a> PostAccess<'a> {
    pub fn new(post: &'a PostAccessView) -> Self {
        Self {
            post,
            access_control: access_control(),
        }
    }

    pub fn can_edit(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control.can(&path, &Permission::Edit)
    }

    pub fn can_view(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control.can(&path, &Permission::View)
    }

    pub fn can_like(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control.can(&path, &Permission::LikePost)
    }

    pub fn can_create_reply(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control.can(&path, &Permission::CreateReply)
    }

    pub fn can_create_public_task(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePublicTask)
    }

    pub fn can_create_private_task(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_post(self.post, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePrivateTask)
    }
}
