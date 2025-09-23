use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::access::base::{
    path::AccessPath, permission::Permission, resource::Resource, role::Role,
};

fn collect_paths(
    resources: &HashMap<Resource, RoleNode>,
    prefix: &str,
    output: &mut HashMap<String, Vec<Permission>>,
) {
    for (resource, role_node) in resources {
        let prefix = if prefix.is_empty() {
            resource.to_string()
        } else {
            format!("{}->{}", prefix, resource.to_string())
        };
        for (role, node) in &role_node.roles {
            let key = format!("{}->{}", prefix, role.to_string());
            output.insert(key.clone(), node.permissions.clone());
            collect_paths(&node.resources, &key, output);
        }
    }
}

fn get_roles_by_res_permission(
    data: &HashMap<Resource, RoleNode>,
    resources: &Vec<Resource>,
    permission: &Permission,
    output: &mut HashSet<Role>,
) {
    for (res, role_node) in data {
        for (role, node) in &role_node.roles {
            if resources.contains(res) && node.permissions.contains(permission) {
                output.insert(role.clone());
            } else {
                get_roles_by_res_permission(&node.resources, resources, permission, output);
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Node {
    #[serde(default)]
    pub permissions: Vec<Permission>,
    #[serde(flatten)]
    pub resources: HashMap<Resource, RoleNode>,
}

#[derive(Debug, Deserialize)]
pub struct RoleNode {
    #[serde(flatten)]
    pub roles: HashMap<Role, Node>,
}

#[derive(Debug)]
pub struct AccessControl {
    paths: HashMap<String, Vec<Permission>>,
    root: HashMap<Resource, RoleNode>,
}

impl From<&str> for AccessControl {
    fn from(value: &str) -> Self {
        let root: HashMap<Resource, RoleNode> =
            serde_json::from_str(value).expect("Invalid JSON string for AccessControl");
        let mut paths: HashMap<String, Vec<Permission>> = HashMap::new();
        collect_paths(&root, "", &mut paths);
        AccessControl { paths, root }
    }
}

impl AccessControl {
    pub fn with_default_schema() -> Self {
        let schema_str = include_str!("schema.json");
        AccessControl::from(schema_str)
    }

    pub fn who_can(&self, permission: &Permission) -> Vec<AccessPath> {
        self.paths
            .iter()
            .filter(|v| v.1.contains(&permission))
            .map(|v| AccessPath::from(v.0.as_str()))
            .collect::<Vec<AccessPath>>()
    }

    pub fn which_roles(&self, resource: Vec<Resource>, permission: Permission) -> Vec<Role> {
        let mut output = HashSet::new();
        get_roles_by_res_permission(&self.root, &resource, &permission, &mut output);
        output.into_iter().collect()
    }

    pub fn what_can(&self, path: &AccessPath) -> Vec<Permission> {
        let path_str = path.to_string();
        self.paths
            .iter()
            .find(|v| v.0 == path_str.as_str())
            .map_or(vec![], |v| v.1.clone())
    }

    pub fn can(&self, path: &AccessPath, permission: &Permission) -> bool {
        let path_str = path.to_string();
        self.paths
            .iter()
            .find(|v| v.1.contains(permission) && v.0 == path_str.as_str())
            .is_some()
    }
}

#[cfg(test)]
mod schema_variant_tests {
    use crate::access::base::{access_control, path::AccessPath, permission::Permission};

    #[test]
    fn test_app_guest_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->GUEST");
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0); // APP->GUEST has empty permissions array
    }

    #[test]
    fn test_app_member_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::CreateDiscussion));
        assert_eq!(permissions.len(), 1);
    }

    #[test]
    fn test_discussion_public_guest_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->GUEST->DISCUSSION:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 1);
    }

    #[test]
    fn test_discussion_public_owner_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreatePublicPost));
        assert!(permissions.contains(&Permission::CreatePrivatePost));
        assert!(permissions.contains(&Permission::CreateIdeaPost));
        assert_eq!(permissions.len(), 5);
    }

    #[test]
    fn test_discussion_public_guest_nested_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 1);
    }

    #[test]
    fn test_post_public_guest_under_discussion_public_guest() {
        let ac = access_control();

        let path = AccessPath::from("APP->GUEST->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 1);
    }

    #[test]
    fn test_post_private_owner_under_discussion_public_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreatePublicTask));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert!(permissions.contains(&Permission::AddMember));
        assert!(permissions.contains(&Permission::RemoveMember));
        assert_eq!(permissions.len(), 8);
    }

    #[test]
    fn test_post_private_member_under_discussion_public_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreatePublicTask));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 5);
    }

    #[test]
    fn test_post_public_owner_under_discussion_public_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 5);
    }

    #[test]
    fn test_post_public_guest_under_discussion_public_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 4);
    }

    #[test]
    fn test_post_public_guest_under_member_discussion_guest() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert_eq!(permissions.len(), 4);
    }

    #[test]
    fn test_task_public_owner_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PUBLIC->OWNER",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_guest_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PUBLIC->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_participant_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PUBLIC->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_owner_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->OWNER",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_guest_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_candidate_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->CANDIDATE",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::RejectTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_participant_under_post_private_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_participant_under_post_private_donor() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->DONOR",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_public_owner_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PUBLIC->OWNER",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_guest_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PUBLIC->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_participant_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PUBLIC->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_owner_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PRIVATE->OWNER",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_guest_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PRIVATE->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_candidate_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PRIVATE->CANDIDATE",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::RejectTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_participant_under_post_private_member() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PRIVATE->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_public_owner_under_post_public_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->OWNER->TASK:PUBLIC->OWNER",
        );
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_task_public_guest_under_post_public_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->OWNER->TASK:PUBLIC->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_task_public_participant_under_post_public_owner() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->OWNER->TASK:PUBLIC->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_task_public_guest_under_post_public_guest() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->GUEST->TASK:PUBLIC->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_task_public_participant_under_post_public_guest() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->GUEST->TASK:PUBLIC->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_task_public_guest_under_guest_path() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->GUEST->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST->TASK:PUBLIC->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 1);
    }

    #[test]
    fn test_task_public_guest_under_member_discussion_guest() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST->TASK:PRIVATE->GUEST",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_public_participant_under_member_discussion_guest() {
        let ac = access_control();

        let path = AccessPath::from(
            "APP->MEMBER->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST->TASK:PRIVATE->PARTICIPANT",
        );
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 2);
    }

    // DISCUSSION:PRIVATE tests
    #[test]
    fn test_discussion_private_owner_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreatePublicPost));
        assert!(permissions.contains(&Permission::CreatePrivatePost));
        assert!(permissions.contains(&Permission::CreatePublicTask));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::AddMember));
        assert!(permissions.contains(&Permission::RemoveMember));
        assert!(permissions.contains(&Permission::Alias));
        assert_eq!(permissions.len(), 9);
    }

    #[test]
    fn test_discussion_private_member_permissions() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreatePublicPost));
        assert!(permissions.contains(&Permission::CreatePrivatePost));
        assert!(permissions.contains(&Permission::CreatePublicTask));
        assert!(permissions.contains(&Permission::CreatePrivateTask));
        assert!(permissions.contains(&Permission::Alias));
        assert_eq!(permissions.len(), 6);
    }

    // Private discussion POST tests
    #[test]
    fn test_post_private_owner_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->POST:PRIVATE->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert!(permissions.contains(&Permission::AddMember));
        assert!(permissions.contains(&Permission::RemoveMember));
        assert_eq!(permissions.len(), 6);
    }

    #[test]
    fn test_post_private_member_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->POST:PRIVATE->MEMBER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_post_private_member_under_discussion_private_member() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->POST:PRIVATE->MEMBER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_post_public_owner_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->POST:PUBLIC->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 4);
    }

    #[test]
    fn test_post_public_guest_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->POST:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_post_public_guest_under_discussion_private_member() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->POST:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::CreateReply));
        assert!(permissions.contains(&Permission::Like));
        assert_eq!(permissions.len(), 3);
    }

    // Private discussion TASK tests
    #[test]
    fn test_task_private_owner_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PRIVATE->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_guest_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PRIVATE->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_candidate_under_discussion_private_owner() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PRIVATE->CANDIDATE");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::RejectTask));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_participant_under_discussion_private_owner() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PRIVATE->PARTICIPANT");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::DeliverTask));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_private_candidate_under_discussion_private_member() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PRIVATE->CANDIDATE");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::RejectTask));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_private_participant_under_discussion_private_member() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PRIVATE->PARTICIPANT");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::DeliverTask));
        assert!(permissions.contains(&Permission::View));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_public_owner_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PUBLIC->OWNER");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::Edit));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_guest_under_discussion_private_owner() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_participant_under_discussion_private_owner() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PUBLIC->PARTICIPANT");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::DeliverTask));
        assert_eq!(permissions.len(), 2);
    }

    #[test]
    fn test_task_public_guest_under_discussion_private_member() {
        let ac = access_control();

        let path = AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PUBLIC->GUEST");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::AcceptTask));
        assert!(permissions.contains(&Permission::Donate));
        assert_eq!(permissions.len(), 3);
    }

    #[test]
    fn test_task_public_participant_under_discussion_private_member() {
        let ac = access_control();

        let path =
            AccessPath::from("APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PUBLIC->PARTICIPANT");
        let permissions = ac.what_can(&path);
        assert!(permissions.contains(&Permission::View));
        assert!(permissions.contains(&Permission::DeliverTask));
        assert_eq!(permissions.len(), 2);
    }

    // Comprehensive permission analysis tests
    #[test]
    fn test_who_can_deliver_task_comprehensive() {
        let ac = access_control();

        let paths = ac.who_can(&Permission::DeliverTask);

        // Verify all expected paths that can deliver tasks
        let expected_paths = vec![
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PUBLIC->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PUBLIC->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->OWNER->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PRIVATE->MEMBER->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PUBLIC->OWNER->POST:PUBLIC->OWNER->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PUBLIC->GUEST->POST:PUBLIC->GUEST->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PRIVATE->OWNER->TASK:PUBLIC->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PRIVATE->PARTICIPANT",
            "APP->MEMBER->DISCUSSION:PRIVATE->MEMBER->TASK:PUBLIC->PARTICIPANT",
        ];

        let path_strings: Vec<String> = paths.iter().map(|p| p.to_string()).collect();

        for expected in expected_paths {
            assert!(
                path_strings.contains(&expected.to_string()),
                "Missing path that can deliver task: {}",
                expected
            );
        }
    }

    #[test]
    fn test_who_can_accept_task_comprehensive() {
        let ac = access_control();

        let paths = ac.who_can(&Permission::AcceptTask);

        // Should include all GUEST and CANDIDATE roles that can accept tasks
        assert!(!paths.is_empty());

        let path_strings: Vec<String> = paths.iter().map(|p| p.to_string()).collect();

        // Verify some key paths exist
        let has_guest_accept = path_strings.iter().any(|p| p.contains("GUEST"));
        let has_candidate_accept = path_strings.iter().any(|p| p.contains("CANDIDATE"));

        assert!(
            has_guest_accept || has_candidate_accept,
            "Should have at least one GUEST or CANDIDATE that can accept tasks. Found paths: {:?}",
            path_strings
        );

        // Verify specific expected paths exist
        assert!(
            path_strings
                .iter()
                .any(|p| p.contains("TASK:PUBLIC->GUEST") || p.contains("TASK:PRIVATE->CANDIDATE")),
            "Should have specific task acceptance paths"
        );
    }

    #[test]
    fn test_who_can_edit_comprehensive() {
        let ac = access_control();
        let paths = ac.who_can(&Permission::Edit);
        let path_strings: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
        for path_str in path_strings {
            assert!(
                path_str.contains("OWNER") || path_str.contains("EDITOR"),
                "Non-owner role found with edit permission: {}",
                path_str
            );
        }
    }

    #[test]
    fn test_who_can_donate_comprehensive() {
        let ac = access_control();
        let paths = ac.who_can(&Permission::Donate);
        let path_strings: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
        for path_str in path_strings {
            assert!(
                path_str.contains("OWNER")
                    || path_str.contains("GUEST")
                    || path_str.contains("DONOR")
            );
        }
    }

    #[test]
    fn test_schema_completeness() {
        let ac = access_control();

        // Verify we have parsed all expected paths
        let all_permissions = [
            Permission::View,
            Permission::Edit,
            Permission::CreatePublicPost,
            Permission::CreatePrivatePost,
            Permission::CreatePublicTask,
            Permission::CreatePrivateTask,
            Permission::CreateDiscussion,
            Permission::AddMember,
            Permission::RemoveMember,
            Permission::AcceptTask,
            Permission::RejectTask,
            Permission::DeliverTask,
            Permission::Like,
            Permission::CreateReply,
            Permission::Donate,
        ];

        for permission in all_permissions {
            let paths = ac.who_can(&permission);
            assert!(
                !paths.is_empty(),
                "No paths found for permission: {:?}",
                permission
            );
        }
    }
}
