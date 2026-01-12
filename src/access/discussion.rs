use crate::{
    access::base::{
        access_control, control::AccessControl, path::AccessPath, permission::Permission,
        resource::Resource, role::Role,
    },
    entities::{
        community::discussion_entity::DiscussionType,
        user_auth::local_user_entity::{LocalUser, UserRole},
    },
    models::view::access::DiscussionAccessView,
};

impl AccessPath {
    pub fn from_discussion(
        disc: &DiscussionAccessView,
        user: Option<&LocalUser>,
        next: Option<Box<AccessPath>>,
    ) -> Self {
        let (user_role, disc_role) = match user {
            Some(user) => (
                match user.role {
                    UserRole::Admin => Role::Admin,
                    UserRole::User => Role::Member,
                },
                disc.users
                    .iter()
                    .find(|u| u.user == *user.id.as_ref().unwrap())
                    .map_or(Role::Guest, |v| Role::from(v.role.as_str())),
            ),
            None => (Role::Guest, Role::Guest),
        };

        AccessPath {
            name: Resource::App,
            role: user_role,
            next: Some(Box::new(AccessPath {
                name: match disc.r#type {
                    DiscussionType::Private => Resource::DiscussionPrivate,
                    DiscussionType::Public => Resource::DiscussionPublic,
                },
                role: disc_role,
                next,
            })),
        }
    }
}

pub struct DiscussionAccess<'a> {
    discussion: &'a DiscussionAccessView,
    access_control: &'static AccessControl,
}

impl<'a> DiscussionAccess<'a> {
    pub fn new(discussion: &'a DiscussionAccessView) -> Self {
        Self {
            discussion,
            access_control: access_control(),
        }
    }

    pub fn can_edit(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::Edit)
    }

    pub fn can_set_alias(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::Alias)
    }

    pub fn can_add_member(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::AddMember)
    }

    pub fn can_remove_member(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::RemoveMember)
    }

    pub fn can_view(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::View)
    }

    pub fn can_create_public_post(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePublicPost)
    }

    pub fn can_create_private_post(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePrivatePost)
    }

    pub fn can_idea_post(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control.can(&path, &Permission::CreateIdeaPost)
    }

    pub fn can_create_private_task(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePrivateTask)
    }

    pub fn can_create_public_task(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePublicTask)
    }
    pub fn can_create_post_for_post(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_discussion(self.discussion, Some(&user), None);
        self.access_control
            .can(&path, &Permission::CreatePostForPost)
    }
}
