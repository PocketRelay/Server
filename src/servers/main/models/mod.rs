//! This module contains the packet content models for all the routes in
//! the routes module and any dependencies for those

use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeError,
    reader::TdfReader,
};

pub mod auth;
pub mod game_manager;
pub mod messaging;
pub mod other;
pub mod session;
pub mod stats;
pub mod user_sessions;
pub mod util;

pub struct EmptyModel;

impl Decodable for EmptyModel {
    fn decode(_reader: &mut TdfReader) -> Result<Self, DecodeError> {
        Ok(Self)
    }
}

impl Encodable for EmptyModel {
    fn encode(&self, _writer: &mut blaze_pk::writer::TdfWriter) {}
}
