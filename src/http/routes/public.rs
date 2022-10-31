use actix_web::web::ServiceConfig;
use actix_web::{get, web, HttpResponse, Responder};
use rust_embed::RustEmbed;

/// Public resource content folder
#[derive(RustEmbed)]
#[folder = "resources/public"]
struct PublicContent;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(content);
}

#[get("/content/{filename:.*}")]
async fn content(path: web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    if let Some(file) = PublicContent::get(&path) {
        HttpResponse::Ok()
            .content_type(
                mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .as_ref(),
            )
            .body(file.data.into_owned())
    } else {
        HttpResponse::NotFound().body("Not Found")
    }
}
