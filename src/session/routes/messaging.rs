use crate::{
    session::{
        models::messaging::*,
        packet::{Packet, Response},
        GetPlayerMessage, PushExt, SessionLink,
    },
    state::App,
    utils::components::messaging,
};

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
///     " (0, 0, 0),
///     "STAT": 0,
///     "TARG": (0, 0, 0),
///     "TYPE": 0
/// }
/// ```
///
pub async fn handle_fetch_messages(session: &mut SessionLink) -> Response<FetchMessageResponse> {
    // Request a copy of the player data
    let Ok(Some(player)) = session.send(GetPlayerMessage).await else {
        // Not authenticated return empty count
        return Response(FetchMessageResponse { count: 0 });
    };

    // Message with player name replaced
    let message: String = App::config()
        .menu_message
        .replace("{n}", &player.display_name);

    let notify = Packet::notify(
        messaging::COMPONENT,
        messaging::SEND_MESSAGE,
        MessageNotify {
            message,
            player_id: player.id,
        },
    );

    session.push(notify);
    Response(FetchMessageResponse { count: 1 })
}
