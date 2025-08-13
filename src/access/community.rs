use crate::entities::{
    community::community_entity::Community, user_auth::local_user_entity::LocalUser,
};

pub struct CommunityAccess<'a> {
    pub community: &'a Community,
}

impl<'a> CommunityAccess<'a> {
    pub fn new(community: &'a Community) -> Self {
        Self { community }
    }

    pub fn can_create_discussion(&self, user: &LocalUser) -> bool {
        self.community.created_by == *user.id.as_ref().unwrap()
    }
}
