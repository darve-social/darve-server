use askama::{DynTemplate, Template};

#[allow(dead_code)]
#[derive(Template)]
#[template(path = "nera2/template_form_page.html")]
pub struct ProfileFormPage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    pub(crate) form: Box<dyn DynTemplate>,
}


impl ProfileFormPage {
    pub fn new(
        form: Box<dyn DynTemplate>,
        theme_name: Option<String>,
        short_title: Option<String>,
        long_title: Option<String>,
    ) -> Self {
        ProfileFormPage {
            theme_name: theme_name.clone().unwrap_or("emerald".to_string()),
            window_title: long_title.clone().unwrap_or("Form Page".to_string()),
            nav_top_title: short_title.unwrap_or("Edit".to_string()),
            header_title: long_title.unwrap_or("Form Page".to_string()),
            footer_text: "".to_string(),
            form,
        }
    }
}

#[allow(dead_code)]
#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct TwitchLoginPage {
    // theme_name: String,
    // window_title: String,
    // nav_top_title: String,
    // header_title: String,
    // footer_text: String,
    // pub(crate) form: Box<dyn DynTemplate>,
}
