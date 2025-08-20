use crate::access::base::{resource::Resource, role::Role};

#[derive(Debug, Clone, PartialEq)]
pub struct AccessPath {
    pub name: Resource,
    pub role: Role,
    pub next: Option<Box<AccessPath>>,
}

impl From<&str> for AccessPath {
    fn from(value: &str) -> Self {
        let segments: Vec<&str> = value.split("->").collect();
        if segments.len() % 2 != 0 {
            panic!("Invalid path format: segments must be pairs of resource and role");
        }

        let mut resource_roles: Option<Box<AccessPath>> = None;
        let mut i = segments.len();
        while i >= 2 {
            let resource = Resource::from(segments[i - 2]);
            let role = Role::from(segments[i - 1]);
            let rr = AccessPath {
                name: resource,
                role,
                next: resource_roles,
            };
            resource_roles = Some(Box::new(rr));
            i -= 2;
        }

        match resource_roles {
            Some(rr) => *rr,
            None => panic!("Invalid path format"),
        }
    }
}

impl ToString for AccessPath {
    fn to_string(&self) -> String {
        let mut segments = vec![];
        let mut item = self;
        loop {
            segments = [
                segments,
                [item.name.to_string(), item.role.to_string()].to_vec(),
            ]
            .concat();

            if item.next.is_none() {
                return segments.join("->");
            }

            item = item.next.as_deref().unwrap();
        }
    }
}
