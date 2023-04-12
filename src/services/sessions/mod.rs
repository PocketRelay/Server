//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::{session::Session, utils::types::PlayerID};
use interlink::prelude::*;
use std::collections::HashMap;

/// Service for storing links to authenticated sessions
#[derive(Service)]
pub struct AuthedSessions {
    /// Map of the authenticated players to their session links
    values: HashMap<PlayerID, Link<Session>>,
}

impl AuthedSessions {
    /// Starts a new service returning its link
    pub fn start() -> Link<Self> {
        let this = Self {
            values: Default::default(),
        };
        this.start()
    }
}

/// Message for removing players from the authenticated
/// sessions list
#[derive(Message)]
pub struct RemoveMessage {
    /// The ID of the player to remove
    pub player_id: PlayerID,
}

/// Message for adding a player to the authenticated
/// sessions list
#[derive(Message)]
pub struct AddMessage {
    /// The ID of the player the link belongs to
    pub player_id: PlayerID,
    /// The link to the player session
    pub link: Link<Session>,
}

/// Message for finding a session by a player ID returning
/// a link to the player if one is found
#[derive(Message)]
#[msg(rtype = "Option<Link<Session>>")]
pub struct LookupMessage {
    /// The ID of the player to lookup
    pub player_id: PlayerID,
}

/// Handle messages to add authenticated sessions
impl Handler<AddMessage> for AuthedSessions {
    type Response = ();

    fn handle(&mut self, msg: AddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.insert(msg.player_id, msg.link);
    }
}

/// Handle messages to remove authenticated sessions
impl Handler<RemoveMessage> for AuthedSessions {
    type Response = ();

    fn handle(&mut self, msg: RemoveMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.remove(&msg.player_id);
    }
}

/// Handle messages to lookup authenticated sessions
impl Handler<LookupMessage> for AuthedSessions {
    type Response = Mr<LookupMessage>;

    fn handle(&mut self, msg: LookupMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let value = self.values.get(&msg.player_id).cloned();
        Mr(value)
    }
}
