use crate::error::CtxError;
use askama::Template;
// use askama_axum::axum_core::response::IntoResponse;
use serde::{Deserialize, Serialize};

/*pub trait  Renderable<T>{
    fn render (&self, is_htmx:bool)-> (StatusCode, Html<String>){

        match is_htmx {
            true => (StatusCode::OK, Html(self::<Template>.render().unwrap())),
            false => (StatusCode::OK, Html(serde_json::to_string(self).expect("valid json")))
        }
    }
}*/

/*pub fn render_htmx_or_json<T: Template + Serialize>(is_htmlx: bool, res: T) -> (StatusCode, Html<String>) {
    match is_htmlx {
        true => (StatusCode::OK, render_template(&res)),
        false => (StatusCode::OK, Html(serde_json::to_string(&res).expect("valid json")))
    }
}

fn render_template<T: Template + Serialize>(res: &T) -> String {
    Html(res.render().unwrap()).0
}
*/

/*pub fn get_htmx_or_json_renderer<T: Template + Serialize>(is_htmx: bool) -> impl Fn(T) -> String {
    return move |res: T| -> String {
        match is_htmx {
            true => Html(res.render().unwrap()).0,
            false => Html(serde_json::to_string(&res).expect("valid json")).0
        }
    }
}*/

pub fn htmx_or_json_err_resp(is_htmx: bool, mut api_error: CtxError) -> CtxError {
    api_error.is_htmx = is_htmx;
    api_error
}

/*pub fn to_htmx_or_json_response<T: Template + Serialize>(result: ApiResult<T>, is_htmx: bool) -> impl IntoResponse + Sized {
    result.map(|res| render_htmx_or_json(is_htmx, res)).map_err(|err| htmx_or_json_err_resp(is_htmx, err))
}*/

#[derive(Template, Debug, Serialize, Deserialize, Clone)] // this will generate the code...
#[template(path = "nera2/register_response.html")] // using the template in this path, relative
                                                   // to the `templates` dir in the crate root
pub struct CreatedResponse {
    // the name of the struct can be anything
    // domain: String, // the field name should match the variable name
    // in your template
    pub success: bool,
    pub id: String,
    #[serde(default)]
    pub uri: Option<String>,
}
