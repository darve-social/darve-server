use crate::{
    access::base::{
        access_control, control::AccessControl, path::AccessPath, permission::Permission,
        resource::Resource, role::Role,
    },
    entities::{
        task_request::TaskRequestType, user_auth::local_user_entity::LocalUser,
    },
    models::view::access::TaskAccessView,
};

impl AccessPath {
    pub fn from_task(task: &TaskAccessView, user: Option<&LocalUser>) -> Self {
        let role = match user {
            Some(user) => task
                .users
                .iter()
                .find(|u| u.user == *user.id.as_ref().unwrap())
                .map_or(Role::Guest, |u| Role::from(u.role.as_str())),
            None => Role::Guest,
        };

        let access_path = Some(Box::new(AccessPath {
            name: match task.r#type {
                TaskRequestType::Private => Resource::TaskPrivate,
                TaskRequestType::Public => Resource::TaskPublic,
            },
            role,
            next: None,
        }));

        if let Some(ref post_view) = task.post {
            AccessPath::from_post(post_view, user, access_path)
        } else if let Some(ref disc_view) = task.discussion {
            AccessPath::from_discussion(disc_view, user, access_path)
        } else {
            *access_path.unwrap()
        }
    }
}

pub struct TaskAccess<'a> {
    task: &'a TaskAccessView,
    access_control: &'static AccessControl,
}

impl<'a> TaskAccess<'a> {
    pub fn new(task: &'a TaskAccessView) -> Self {
        Self {
            task,
            access_control: access_control(),
        }
    }

    pub fn can_edit(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::Edit)
    }

    pub fn can_donate(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::Donate)
    }

    pub fn can_accept(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::AcceptTask)
    }

    pub fn can_deliver(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::DeliverTask)
    }

    pub fn can_reject(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::RejectTask)
    }

    pub fn can_view(&self, user: &LocalUser) -> bool {
        let path = AccessPath::from_task(self.task, Some(&user));
        self.access_control.can(&path, &Permission::View)
    }
}
