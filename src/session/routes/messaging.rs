use crate::{
    config::{RuntimeConfig, VERSION},
    session::{
        models::messaging::*,
        packet::Packet,
        router::{Blaze, Extension},
        GetPlayerMessage, PushExt, SessionLink,
    },
    utils::components::messaging,
};
use std::sync::Arc;

/// Handles requests from the client to fetch the server messages. The initial response contains
/// the amount of messages and then each message is sent using a SendMessage notification.
///
/// ```
/// Route: Messaging(FetchMessages)
/// ID: 24
/// Content: {
///     "FLAG": 0,
///     "MGID": 0,
///     "PIDX": 0,
///     "PSIZ": 0,
///     "SMSK": 0,
///     "SORT": 0,
///     "SRCE": (0, 0, 0),
///     "STAT": 0,
///     "TARG": (0, 0, 0),
///     "TYPE": 0
/// }
/// ```
pub async fn handle_fetch_messages(
    session: SessionLink,
    Extension(config): Extension<Arc<RuntimeConfig>>,
) -> Blaze<FetchMessageResponse> {
    // Request a copy of the player data
    let Ok(Some(player)) = session.send(GetPlayerMessage).await else {
        // Not authenticated return empty count
        return Blaze(FetchMessageResponse { count: 0 });
    };

    // Message with player name replaced
    let mut message: String = config
        .menu_message
        .replace("{v}", VERSION)
        .replace("{n}", &player.display_name);
    // Line terminator for the end of the message
    message.push(char::from(0x0A));

    let notify = Packet::notify(
        messaging::COMPONENT,
        messaging::SEND_MESSAGE,
        MessageNotify {
            message,
            player_id: player.id,
        },
    );

    session.push(notify);
    Blaze(FetchMessageResponse { count: 1 })
}
