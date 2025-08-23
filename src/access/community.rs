use crate::{
    access::base::{
        access_control, path::AccessPath, permission::Permission, resource::Resource, role::Role,
    },
    entities::{community::community_entity::Community, user_auth::local_user_entity::LocalUser},
};

pub struct CommunityAccess<'a> {
    community: &'a Community,
}

impl AccessPath {
    pub fn from_user(user: Option<&LocalUser>) -> Self {
        AccessPath {
            name: Resource::App,
            role: match user {
                Some(_) => Role::Member,
                None => Role::Guest,
            },
            next: None,
        }
    }
}

impl<'a> CommunityAccess<'a> {
    pub fn new(community: &'a Community) -> Self {
        Self { community }
    }
    pub fn can_create_discussion(&self, user: &LocalUser) -> bool {
        if &self.community.created_by != user.id.as_ref().unwrap() {
            return false;
        }
        let path = AccessPath::from_user(Some(&user));
        access_control().can(&path, &Permission::CreateDiscussion)
    }
}
