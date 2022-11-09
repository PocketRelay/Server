//! Module for testing different functionality using the official server as a
//! test dummy. (e.g. Modeling requests and finding errors and edge cases)
//
//use blaze_pk::packet;
//use log::info;
//
//use crate::{
//    blaze::components::{Components, Util},
//    utils::init_logger,
//};
//
//use super::Retriever;
//
// Test for determining the response when a client tries to
// load all its settings before is authenticated
//
// Results from this test show that attempting to load settings
// before being authenticated will return a error response with
// the error code: 0x4004
//
//#[tokio::test]
//async fn test_load_all_no_auth() {
//    init_logger();
//
//    std::env::set_var(crate::env::RETRIEVER.0, "true");
//
//    let retriever = Retriever::new().await.expect("Unable to load retriever");
//    let mut session = retriever.session().expect("Unable to create session");
//    let res = session
//        .request_empty_raw(Components::Util(Util::UserSettingsLoadAll))
//        .expect("Failed to get response");
//
//    info!("Type: {:?}", res.0.ty);
//    info!("Error: {}", res.0.error);
//    let contents = res.debug_decode().expect("Failed to decode contents");
//    info!("Contents: {}", contents);
//}
//
//#[tokio::test]
//async fn test_cancel_no_auth() {
//    init_logger();
//
//    std::env::set_var(crate::env::RETRIEVER.0, "true");
//
//    let retriever = Retriever::new().await.expect("Unable to load retriever");
//    let mut session = retriever.session().expect("Unable to create session");
//    let res = session
//        .request_empty_raw(Components::GameManager(
//            crate::blaze::components::GameManager::CancelMatchmaking,
//        ))
//        .expect("Failed to get response");
//
//    info!("Type: {:?}", res.0.ty);
//    info!("Error: {}", res.0.error);
//    let contents = res.debug_decode().expect("Failed to decode contents");
//    info!("Contents: {}", contents);
//}
//
//packet! {
//    struct Silent {
//        AUTH key: String,
//        PID pid: u32,
//        TYPE ty: u8,
//    }
//}
//
//#[tokio::test]
//async fn test_bad_silent() {
//    init_logger();
//
//    std::env::set_var(crate::env::RETRIEVER.0, "true");
//
//    let retriever = Retriever::new().await.expect("Unable to load retriever");
//    let mut session = retriever.session().expect("Unable to create session");
//    let res = session
//        .request_raw(
//            Components::Authentication(crate::blaze::components::Authentication::SilentLogin),
//            &Silent {
//                key: String::from("TesTSTSTASdDAWAW"),
//                pid: 0x1,
//                ty: 0x2,
//            },
//        )
//        .expect("Failed to get response");
//
//    info!("Type: {:?}", res.0.ty);
//    info!("Error: {}", res.0.error);
//    let contents = res.debug_decode().expect("Failed to decode contents");
//    info!("Contents: {}", contents);
//}
