use crate::{
    access::base::{
        access_control, path::AccessPath, permission::Permission, resource::Resource, role::Role,
    },
    entities::user_auth::local_user_entity::LocalUser,
};

pub struct CommunityAccess;

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

impl CommunityAccess {
    pub fn can_create_discussion(user: &LocalUser) -> bool {
        let path = AccessPath::from_user(Some(&user));
        access_control().can(&path, &Permission::CreateDiscussion)
    }
}
