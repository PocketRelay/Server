use self::session::SessionLink;
use crate::utils::components::Components;
use blaze_pk::router::Router;
mod models;
mod routes;
pub mod session;

static mut ROUTER: Option<Router<Components, SessionLink>> = None;

fn router() -> &'static Router<Components, SessionLink> {
    unsafe {
        match &ROUTER {
            Some(value) => value,
            None => panic!("Main server router not yet initialized"),
        }
    }
}

pub fn init_router() {
    unsafe {
        ROUTER = Some(routes::router());
    }
}
