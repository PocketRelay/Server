use actix_web::web::ServiceConfig;
use actix_web::{get, web, HttpResponse, Responder};
use rust_embed::RustEmbed;

/// Public resource content folder
#[derive(RustEmbed)]
#[folder = "src/resources/public"]
struct PublicContent;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(content);
}

/// Function for serving content from the embedded public
/// content. Directory structure matches the paths vistied
/// in this url.
///
/// `path` The path of the content to serve
#[get("/content/{filename:.*}")]
async fn content(path: web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    if let Some(file) = PublicContent::get(&path) {
        HttpResponse::Ok()
            .content_type(mime_guess::from_path(&path).first_or_text_plain().as_ref())
            .body(file.data.into_owned())
    } else {
        HttpResponse::NotFound().body("Not Found")
    }
}
