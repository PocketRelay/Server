use actix_web::{HttpResponse, Responder, get};


#[get("/")]
pub async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}
