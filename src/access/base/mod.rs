use crate::access::base::control::AccessControl;
use std::sync::LazyLock;

pub mod control;
pub mod path;
pub mod permission;
pub mod resource;
pub mod role;

static GLOBAL_ACCESS_CONTROL: LazyLock<AccessControl> =
    LazyLock::new(|| AccessControl::with_default_schema());

pub fn access_control() -> &'static AccessControl {
    &GLOBAL_ACCESS_CONTROL
}
