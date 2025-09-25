pub enum SystemTags {
    Delivery,
}

impl SystemTags {
    pub fn as_str(&self) -> &'static str {
        match self {
            SystemTags::Delivery => "_delivery",
        }
    }
}

impl TryFrom<&str> for SystemTags {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "_delivery" => Ok(SystemTags::Delivery),
            _ => Err(()),
        }
    }
}
