use blaze_pk::{
    codec::Codec,
    packet::{Packet, PacketComponents},
    tag::ValueType,
    tagging::*,
    types::encode_str,
};
use core::blaze::errors::HandleResult;

use crate::session::Session;
use core::blaze::components::{Components, Messaging, UserSessions};

use core::{env, env::VERSION};
use log::debug;
use utils::{time::server_unix_time, types::PlayerID};

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &mut Session, component: Messaging, packet: &Packet) -> HandleResult {
    match component {
        Messaging::FetchMessages => handle_fetch_messages(session, packet).await,
        component => {
            debug!("Got Messaging({component:?})");
            session.response_empty(packet).await
        }
    }
}

#[derive(Debug)]
struct MessageCount {
    count: u8,
}

impl Codec for MessageCount {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "MCNT", self.count)
    }
}

#[derive(Debug)]
struct MenuMessage {
    message: String,
    player_id: PlayerID,
    time: u64,
}

impl Codec for MenuMessage {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "FLAG", 0x1);
        tag_u8(output, "MGID", 0x1);
        tag_str(output, "NAME", &self.message);

        let ref_value = Components::UserSessions(UserSessions::SetSession).values();
        let player_ref = (ref_value.0, ref_value.1, self.player_id);

        {
            tag_group_start(output, "PYLD");
            {
                tag_map_start(output, "ATTR", ValueType::String, ValueType::String, 1);
                encode_str("B0000", output);
                encode_str("160", output);
            }
            tag_u8(output, "FLAG", 0x1);
            tag_u8(output, "STAT", 0x0);
            tag_u8(output, "TAG", 0x0);
            tag_triple(output, "TARG", &player_ref);
            tag_u8(output, "TYPE", 0x0);
            tag_group_end(output);
        }
        tag_triple(output, "SRCE", &player_ref);
        tag_u64(output, "TIME", self.time)
    }
}

/// Handles requests from the client to fetch the server messages. The initial response contains
/// the amount of messages and then each message is sent using a SendMessage notification.
///
/// # Structure
/// ```
/// packet(Components.MESSAGING, Commands.FETCH_MESSAGES, 0x18) {
///   number("FLAG", 0x0)
///   number("MGID", 0x0)
///   number("PIDX", 0x0)
///   number("PSIZ", 0x0)
///   number("SMSK", 0x0)
///   number("SORT", 0x0)
///   tripple("SRCE", 0x0, 0x0, 0x0)
///   number("STAT", 0x0)
///   tripple("TARG", 0x0, 0x0, 0x0)
///   number("TYPE", 0x0)
/// }
/// ```
///
async fn handle_fetch_messages(session: &mut Session, packet: &Packet) -> HandleResult {
    let (player_name, player_id) = match session.player.as_ref() {
        Some(player) => (player.display_name.clone(), player.id),
        None => {
            // Not authenticated return empty count
            return session.response(packet, &MessageCount { count: 0 }).await;
        }
    };
    session.response(packet, &MessageCount { count: 1 }).await?;
    let time = server_unix_time();
    let menu_message = get_menu_message(session, player_name);
    let response = MenuMessage {
        message: menu_message,
        player_id,
        time,
    };

    session
        .notify_immediate(Components::Messaging(Messaging::SendMessage), &response)
        .await?;
    Ok(())
}

/// Retrieves the menu message from the environment variables and replaces
/// any variables inside the message with the correct values for this session
///
/// # Variables
/// - {v} = Server Version
/// - {n} = Player Display Name
/// - {ip} = Session IP Address
fn get_menu_message(session: &mut Session, player_name: String) -> String {
    let mut message = env::str_env(env::MENU_MESSAGE);
    if message.contains("{v}") {
        message = message.replace("{v}", VERSION);
    }
    if message.contains("{n}") {
        message = message.replace("{n}", &player_name);
    }
    if message.contains("{ip}") {
        message = message.replace("{ip}", &session.addr.to_string());
    }
    // Line terminator for the end of the message
    message.push(char::from(0x0A));
    message
}
