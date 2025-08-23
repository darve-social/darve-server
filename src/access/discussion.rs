use crate::entities::{
    community::discussion_entity::{Discussion, DiscussionDenyRule},
    user_auth::local_user_entity::LocalUser,
};

pub struct DiscussionAccess<'a> {
    pub discussion: &'a Discussion,
}

impl<'a> DiscussionAccess<'a> {
    pub fn new(discussion: &'a Discussion) -> Self {
        Self { discussion }
    }

    pub fn can_create_task(&self, user: &LocalUser) -> bool {
        let is_deny = self.discussion.deny_rules.as_ref().map_or(false, |rules| {
            rules.contains(&DiscussionDenyRule::CreateTask)
        });

        if is_deny {
            return false;
        }

        self.discussion.is_owner(&user.id.as_ref().unwrap())
            || self.discussion.is_member(&user.id.as_ref().unwrap())
    }

    pub fn can_create_post(&self, user: &LocalUser) -> bool {
        self.discussion.is_owner(&user.id.as_ref().unwrap())
            || self.discussion.is_member(&user.id.as_ref().unwrap())
    }

    pub fn can_manage_members(&self, user: &LocalUser) -> bool {
        let is_deny = self.discussion.deny_rules.as_ref().map_or(false, |rules| {
            rules.contains(&DiscussionDenyRule::ManageMember)
        });

        if is_deny {
            return false;
        }

        self.discussion.is_owner(&user.id.as_ref().unwrap())
    }

    pub fn can_edit(&self, user: &LocalUser) -> bool {
        self.discussion.is_owner(&user.id.as_ref().unwrap())
    }

    pub fn can_view(&self, user: &LocalUser) -> bool {
        self.discussion.is_profile()
            || self.discussion.is_owner(&user.id.as_ref().unwrap())
            || self.discussion.is_member(&user.id.as_ref().unwrap())
    }
}
