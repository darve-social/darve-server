use crate::routes::webauthn::webauthn_routes::WebauthnConfig;
use std::collections::HashMap;
use std::sync::Arc;
use webauthn_rs::prelude::*;

/*
 * Webauthn RS server side app state and setup  code.
 */

// Configure the Webauthn instance by using the WebauthnBuilder. This defines
// the options needed for your site, and has some implications. One of these is that
// you can NOT change your rp_id (relying party id), without invalidating all
// webauthn credentials. Remember, rp_id is derived from your URL origin, meaning
// that it is your effective domain name.

pub struct Data {
    pub user_ident_to_uuid: HashMap<String, Uuid>,
    pub keys: HashMap<Uuid, Vec<Passkey>>,
}

#[derive(Clone)]
pub struct AppState {
    // Webauthn has no mutable inner state, so Arc and read only is sufficent.
    // Alternately, you could use a reference here provided you can work out
    // lifetimes.
    pub webauthn: Arc<Webauthn>,
    // This needs mutability, so does require a mutex.
    // pub users: Arc<Mutex<Data>>,
}

impl AppState {
    pub fn new(wa_config: WebauthnConfig) -> Self {
        // Effective domain name.
        // let relaying_party_domain = "localhost";
        // Url containing the effective domain name
        // MUST include the port number!
        // let wa_origin_url = "http://localhost:8080";
        let rp_origin =
            Url::parse(wa_config.relaying_party_origin_url.as_str()).expect("Invalid URL");
        let builder = WebauthnBuilder::new(wa_config.relaying_party_domain.as_str(), &rp_origin)
            .expect("Invalid configuration");

        // Now, with the builder you can define other options.
        // Set a "nice" relying party name. Has no security properties and
        // may be changed in the future.
        // let wa_relay_name = "NewEra Network";
        let builder = builder.rp_name(wa_config.relaying_party_name.as_str());

        // Consume the builder and create our webauthn instance.
        let webauthn = Arc::new(builder.build().expect("Invalid configuration"));

        /* let users = Arc::new(Mutex::new(Data {
            user_ident_to_uuid: HashMap::new(),
            keys: HashMap::new(),
        }));*/

        AppState {
            webauthn, /*, users*/
        }
    }
}
