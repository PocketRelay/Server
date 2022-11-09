use crate::blaze::components::{Components, Messaging, UserSessions};
use crate::blaze::errors::HandleResult;
use crate::blaze::session::SessionArc;
use crate::database::entities::PlayerModel;
use crate::env;
use crate::env::VERSION;
use crate::utils::server_unix_time;
use blaze_pk::{
    encode_str, tag_group_end, tag_group_start, tag_map_start, tag_str, tag_triple, tag_u64,
    tag_u8, Codec, OpaquePacket, PacketComponents, ValueType,
};
use log::debug;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(
    session: &SessionArc,
    component: Messaging,
    packet: &OpaquePacket,
) -> HandleResult {
    match component {
        Messaging::FetchMessages => handle_fetch_messages(session, packet).await,
        component => {
            debug!("Got Messaging({component:?})");
            packet.debug_decode()?;
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
struct MenuMessage<'a> {
    message: String,
    player: &'a PlayerModel,
    time: u64,
}

impl Codec for MenuMessage<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u8(output, "FLAG", 0x1);
        tag_u8(output, "MGID", 0x1);
        tag_str(output, "NAME", &self.message);

        let ref_value = Components::UserSessions(UserSessions::SetSession).values();
        let player_ref = (ref_value.0, ref_value.1, self.player.id);

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
async fn handle_fetch_messages(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let session_data = session.data.read().await;
    let player = match session_data.player.as_ref() {
        Some(player) => player,
        None => {
            // Not authenticated return empty count
            return session.response(packet, &MessageCount { count: 0 }).await;
        }
    };
    session.response(packet, &MessageCount { count: 1 }).await?;
    let time = server_unix_time();
    let menu_message = get_menu_message(session, player);
    let response = MenuMessage {
        message: menu_message,
        player,
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
fn get_menu_message(session: &SessionArc, player: &PlayerModel) -> String {
    let mut message = env::str_env(env::MENU_MESSAGE);
    if message.contains("{v}") {
        message = message.replace("{v}", VERSION);
    }
    if message.contains("{n}") {
        message = message.replace("{n}", &player.display_name);
    }
    if message.contains("{ip}") {
        message = message.replace("{ip}", &session.addr.to_string());
    }
    // Line terminator for the end of the message
    message.push(char::from(0x0A));
    message
}
