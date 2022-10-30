use blaze_pk::{OpaquePacket, packet, TdfMap};
use log::debug;
use crate::blaze::components::GameManager;
use crate::blaze::errors::{BlazeError, GameError, HandleResult};
use crate::blaze::SessionArc;
use crate::game::Game;

/// Routing function for handling packets with the `GameManager` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &SessionArc, component: GameManager, packet: &OpaquePacket) -> HandleResult {
    match component {
        GameManager::CreateGame => handle_create_game(session, packet).await,
        GameManager::AdvanceGameState => handle_advance_game_state(session, packet).await,
        GameManager::SetGameSettings => handle_set_game_setting(session, packet).await,
        component => {
            debug!("Got GameManager({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}

packet! {
    struct CreateGameReq {
        ATTR attributes: TdfMap<String, String>,
        GSET setting: u16,
        GNAM name: String
    }
}

packet! {
    struct CreateGameRes {
        GID id: u32,
    }
}

/// Handles creating a game for the provided session.
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.CREATE_GAME, 0x37) {
///   map("ATTR", mapOf(
///     "ME3_dlc2300" to "required",
///     "ME3_dlc2500" to "required",
///     "ME3_dlc2700" to "required",
///     "ME3_dlc3050" to "required",
///     "ME3_dlc3225" to "required",
///     "ME3gameDifficulty" to "difficulty0",
///     "ME3gameEnemyType" to "enemy1",
///     "ME3map" to "map11",
///     "ME3privacy" to "PUBLIC",
///   ))
///   tripple("BTPL", 0x0, 0x0, 0x0)
///   text("GCTR", "")
///   number("GENT", 0x0)
///   text("GNAM", "test@test.com")
///   number("GSET", 0x11f)
///   text("GTYP", "")
///   text("GURL", "")
///   list("HNET", listOf(
///     group(start2=true) {
///       +group("EXIP") {
///         number("IP", 0x0)
///         number("PORT", 0xe4b)
///       }
///       +group("INIP") {
///         number("IP", 0xc0a8014a)
///         number("PORT", 0xe4b)
///       }
///     }
///   ))
///   number("IGNO", 0x0)
///   number("NRES", 0x0)
///   number("NTOP", 0x0)
///   list("PCAP", listOf(0x4, 0x0))
///   text("PGID", "")
///   blob("PGSC")
///   number("PMAX", 0x4)
///   number("PRES", 0x1)
///   number("QCAP", 0x0)
///   number("RGID", 0x0)
///   number("SLOT", 0x0)
///   number("TCAP", 0x0)
///   number("TIDX", 0xffff)
///   number("VOIP", 0x2)
///   text("VSTR", "ME3-295976325-179181965240128")
/// }
/// ```
async fn handle_create_game(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<CreateGameReq>()?;

    let game = session.games().new_game(
        req.name,
        req.attributes,
        req.setting,
    ).await;

    session.response(packet, &CreateGameRes { id: game.id }).await?;
    Game::add_player(&game, session).await?;

    // TODO: Update matchmaking await.

    Ok(())
}

packet! {
    struct GameStateReq {
        GID id: u32,
        GSTA state: u16,
    }
}

/// Handles changing the state of the game with the provided ID
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.ADVANCE_GAME_STATE, 0x39) {
///   number("GID", 0x5dc695)
///   number("GSTA", 0x82)
/// }
/// ```
///
async fn handle_advance_game_state(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<GameStateReq>()?;
    let game = session.games()
        .find_by_id(req.id)
        .await
        .ok_or_else(|| BlazeError::Game(GameError::UnknownGame(req.id)))?;
    game.set_state(req.state).await?;
    session.response_empty(packet).await
}

packet! {
    struct GameSettingReq {
        GID id: u32,
        GSET setting: u16,
    }
}


/// Handles changing the setting of the game with the provided ID
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.SET_GAME_SETTINGS, 0xa1) {
///   number("GID", 0x48a759)
///   number("GSET", 0x11d)
/// }
/// ```
///
async fn handle_set_game_setting(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<GameSettingReq>()?;
    let game = session.games()
        .find_by_id(req.id)
        .await
        .ok_or_else(|| BlazeError::Game(GameError::UnknownGame(req.id)))?;
    game.set_setting(req.setting).await?;
    session.response_empty(packet).await
}
