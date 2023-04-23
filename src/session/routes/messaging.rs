use crate::{
    session::{models::messaging::*, GetPlayerMessage, PushExt, SessionLink},
    state::App,
    utils::components::{Components as C, Messaging as M},
};
use blaze_pk::{packet::Packet, router::Router};

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionLink>) {
    router.route(C::Messaging(M::FetchMessages), handle_fetch_messages);
}

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
async fn handle_fetch_messages(session: &mut SessionLink) -> FetchMessageResponse {
    // Request a copy of the player data
    let Ok(Some(player)) = session.send(GetPlayerMessage).await else {
        // Not authenticated return empty count
        return FetchMessageResponse { count: 0 };
    };

    // Message with player name replaced
    let message: String = App::config()
        .menu_message
        .replace("{n}", &player.display_name);

    let notify = Packet::notify(
        C::Messaging(M::SendMessage),
        MessageNotify {
            message,
            player_id: player.id,
        },
    );

    session.push(notify);
    FetchMessageResponse { count: 1 }
}
