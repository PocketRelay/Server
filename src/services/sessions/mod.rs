//! Service for storing links to all the currenly active
//! authenticated sessions on the server

use crate::{servers::main::session::Session, utils::types::PlayerID};
use interlink::prelude::*;
use std::collections::HashMap;

#[derive(Service)]
pub struct AuthedSessions {
    values: HashMap<PlayerID, Link<Session>>,
}

impl AuthedSessions {
    /// Starts a new matchmaking service returning its link
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
    pub player_id: PlayerID,
}

/// Message for adding a player to the authenticated
/// sessions list
#[derive(Message)]
pub struct AddMessage {
    pub player_id: PlayerID,
    pub link: Link<Session>,
}

/// Message for finding a session by a player ID returning
/// a link to the player if one is found
#[derive(Message)]
#[msg(rtype = "Option<Link<Session>>")]
pub struct LookupMessage {
    pub player_id: PlayerID,
}

impl Handler<AddMessage> for AuthedSessions {
    type Response = ();

    fn handle(&mut self, msg: AddMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.insert(msg.player_id, msg.link);
    }
}

impl Handler<RemoveMessage> for AuthedSessions {
    type Response = ();

    fn handle(&mut self, msg: RemoveMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        self.values.remove(&msg.player_id);
    }
}

impl Handler<LookupMessage> for AuthedSessions {
    type Response = Mr<LookupMessage>;

    fn handle(&mut self, msg: LookupMessage, _ctx: &mut ServiceContext<Self>) -> Self::Response {
        let value = self.values.get(&msg.player_id).cloned();
        Mr(value)
    }
}
