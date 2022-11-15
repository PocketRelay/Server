use blaze_pk::{group, packet, packet::Packet, types::TdfMap};
use core::blaze::components::GameManager;
use core::blaze::errors::{HandleResult, ServerError};
use core::blaze::session::SessionArc;
use core::game::rules::{MatchRules, RuleSet};
use log::{debug, info, warn};

/// Routing function for handling packets with the `GameManager` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &SessionArc, component: GameManager, packet: &Packet) -> HandleResult {
    match component {
        GameManager::CreateGame => handle_create_game(session, packet).await,
        GameManager::AdvanceGameState => handle_advance_game_state(session, packet).await,
        GameManager::SetGameSettings => handle_set_game_setting(session, packet).await,
        GameManager::SetGameAttributes => handle_set_game_attribs(session, packet).await,
        GameManager::RemovePlayer => handle_remove_player(session, packet).await,
        GameManager::UpdateMeshConnection => handle_update_mesh_connection(session, packet).await,
        GameManager::StartMatchaking => handle_start_matchmaking(session, packet).await,
        GameManager::CancelMatchmaking => handle_cancel_matchmaking(session, packet).await,
        component => {
            debug!("Got GameManager({component:?})");
            session.response_empty(packet).await
        }
    }
}

packet! {
    struct CreateGameReq {
        ATTR attributes: TdfMap<String, String>,
        GNAM name: String,
        GSET setting: u16,
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
///         number("PORT", 0x0)
///       }
///       +group("INIP") {
///         number("IP", 0x0)
///         number("PORT", 0x0)
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
async fn handle_create_game(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<CreateGameReq>()?;

    let games = session.games();

    let game_id = games.create_game(req.attributes, req.setting).await;

    session
        .response(packet, &CreateGameRes { id: game_id })
        .await?;

    games.add_host(game_id, session).await;

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
async fn handle_advance_game_state(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<GameStateReq>()?;
    let games = session.games();
    if games.set_game_state(req.id, req.state).await {
        session.response_empty(packet).await
    } else {
        warn!(
            "Client requested to advance the game state of an unknown game (GID: {}, SID: {})",
            req.id, session.id
        );
        session
            .response_error_empty(packet, ServerError::InvalidInformation)
            .await
    }
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
async fn handle_set_game_setting(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<GameSettingReq>()?;

    let games = session.games();
    if games.set_game_setting(req.id, req.setting).await {
        session.response_empty(packet).await
    } else {
        warn!(
            "Client requested to set the game setting of an unknown game (GID: {}, SID: {})",
            req.id, session.id
        );
        session
            .response_error_empty(packet, ServerError::InvalidInformation)
            .await
    }
}

packet! {
    struct GameAttribsReq {
        ATTR attributes: TdfMap<String, String>,
        GID id: u32,
    }
}

/// Handles changing the attributes of the game with the provided ID
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.SET_GAME_ATTRIBUTES, 0xa2) {
///   map("ATTR", mapOf(
///     "ME3_dlc2300" to "required",
///     "ME3_dlc2500" to "required",
///     "ME3_dlc2700" to "required",
///     "ME3_dlc3050" to "required",
///     "ME3_dlc3225" to "required",
///     "ME3gameDifficulty" to "difficulty0",
///     "ME3gameEnemyType" to "enemy1",
///     "ME3map" to "map2",
///     "ME3privacy" to "PUBLIC",
///   ))
///   number("GID", 0x48a759)
/// }
/// ```
///
async fn handle_set_game_attribs(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<GameAttribsReq>()?;

    let games = session.games();
    if games.set_game_attributes(req.id, req.attributes).await {
        session.response_empty(packet).await
    } else {
        warn!(
            "Client requested to set the game attributes of an unknown game (GID: {}, SID: {})",
            req.id, session.id
        );
        session
            .response_error_empty(packet, ServerError::InvalidInformation)
            .await
    }
}

packet! {
    struct RemovePlayerReq {
        GID id: u32,
        PID pid: u32,
    }
}

/// Handles removing a player from a game
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.REMOVE_PLAYER, 0x97) {
///   triple("BTPL", 0x0, 0x0, 0x0)
///   number("CNTX", 0x0)
///   number("GID", 0x48a758)
///   number("PID", 0x3a5508eb)
///   number("REAS", 0x6)
/// }
/// ```
async fn handle_remove_player(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<RemovePlayerReq>()?;
    let games = session.games();

    if games.remove_player_pid(req.id, req.pid).await {
        session.response_empty(packet).await
    } else {
        warn!(
            "Client requested to advance the game state of an unknown game (GID: {}, SID: {})",
            req.id, session.id
        );
        session
            .response_error_empty(packet, ServerError::InvalidInformation)
            .await
    }
}

packet! {
    struct UpdateMeshReq {
        GID id: u32,
        TARG targets: Vec<MeshTarget>,
    }
}

group! {
    struct MeshTarget {
        PID id: u32,
    }
}

/// Handles updating mesh connections
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.UPDATE_MESH_CONNECTION, 0x93) {
///   number("GID", 0x48a758)
///   list("TARG", listOf(
///     group {
///       number("FLGS", 0x0)
///       number("PID", 0xccc456b)
///       number("STAT", 0x2)
///     }
///   ))
/// }
/// ```
async fn handle_update_mesh_connection(session: &SessionArc, packet: &Packet) -> HandleResult {
    session.response_empty(packet).await?;

    let req = packet.decode::<UpdateMeshReq>()?;

    let Some(target) = req.targets.first() else {
        return Ok(())
    };

    let games = session.games();

    if !games
        .update_mesh_connection(req.id, session, target.id)
        .await
    {
        warn!(
            "Client requested to advance the game state of an unknown game (GID: {}, SID: {})",
            req.id, session.id
        );
    }

    Ok(())
}

packet! {
    struct MatchmakingReq {
        CRIT criteria: MatchCriteria,
    }
}

group! {
    struct MatchCriteria {
        RLST rules: Vec<Rule>
    }
}

group! {
    struct Rule {
        NAME name: String,
        VALU value: Vec<String>,
    }
}

fn parse_ruleset(rules: Vec<Rule>) -> RuleSet {
    let mut out = Vec::new();
    for rule in rules {
        let Some(value) = rule.value.first() else {
            continue;
        };
        if let Some(match_rule) = MatchRules::parse(&rule.name, value) {
            out.push(match_rule);
        }
    }
    RuleSet::new(out)
}

packet! {
    struct MatchmakingRes {
        MSID id: u32,
    }
}

/// Handles either directly joining a game or placing the
/// session into a matchmaking queue for searching for games.
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.START_MATCHMAKING, 0x92) {
///  tripple("BTPL", 0x0, 0x0, 0x0)
///  +group("CRIT") {
///    +group("CUST") {}
///     +group("DNF") { number("DNF", 0x65) }
///     +group("GEO") { text("THLD", "") }
///     +group("GNAM") { text("SUBS", "") }
///     +group("NAT") { text("THLD", "hostBalancing")   }
///     +group("PSR") { text("THLD", "") }
///     +group("RANK") { text("THLD", "") }
///     list("RLST", listOf(
///       group {
///         text("NAME", "ME3_gameStateMatchRule")
///         text("THLD", "quickMatch")
///         list("VALU", listOf("MATCH_MAKING"))
///       },
///       group {
///         text("NAME", "ME3_gameMapMatchRule")
///         text("THLD", "quickMatch")
///         list("VALU", listOf("abstain"))
///       },
///       group {
///         text("NAME", "ME3_gameEnemyTypeRule")
///         text("THLD", "quickMatch")
///         list("VALU", listOf("abstain"))
///       },
///       group {
///         text("NAME", "ME3_gameDifficultyRule")
///         text("THLD", "quickMatch")
///         list("VALU", listOf("abstain"))
///       },
///       group {
///         text("NAME", "ME3_rule_dlc2500")
///         text("THLD", "requireExactMatch")
///         list("VALU", listOf("required"))
///       },
///       group {
///         text("NAME", "ME3_rule_dlc2300")
///         text("THLD", "requireExactMatch")
///         list("VALU", listOf("required"))
///       },
///       group {
///         text("NAME", "ME3_rule_dlc2700")
///         text("THLD", "requireExactMatch")
///         list("VALU", listOf("required"))
///       },
///       group {
///         text("NAME", "ME3_rule_dlc3050")
///         text("THLD", "requireExactMatch")
///         list("VALU", listOf("required"))
///       },
///       group {
///         text("NAME", "ME3_rule_dlc3225")
///         text("THLD", "requireExactMatch")
///         list("VALU", listOf("required"))
///       }
///     ))
///     +group("RSZR") {
///       number("PCAP", 0xffff)
///       number("PMIN", 0x0)
///     }
///     +group("SIZE") {
///       number("ISSG", 0x0)
///       number("PCAP", 0x4)
///       number("PCNT", 0x4)
///       number("PMIN", 0x2)
///       text("THLD", "matchAny")
///     }
///     +group("TEAM") {
///       number("PCAP", 0x0)
///       number("PCNT", 0x0)
///       number("PMIN", 0x0)
///       number("SDIF", 0x0)
///       text("THLD", "")
///       number("TID", 0xffff)
///     }
///     map("UED", mapOf(
///       "ME3_characterSkill_Rule" to       group {
///         number("CVAL", 0x0)
///         text("NAME", "ME3_characterSkill_Rule")
///         number("OVAL", 0x0)
///         text("THLD", "quickMatch")
///       },
///     ))
///     +group("VIAB") {
///       text("THLD", "hostViability")
///     }
///     +group("VIRT") {
///       text("THLD", "")
///       number("VALU", 0x1)
///     }
///   }
///   number("DUR", 0x1b7740)
///   number("GENT", 0x0)
///   text("GNAM", "")
///   number("GSET", 0x51f)
///   text("GVER", "ME3-295976325-179181965240128")
///   number("IGNO", 0x0)
///   number("MODE", 0x3)
///   number("NTOP", 0x0)
///   number("PMAX", 0x0)
///   optional("PNET",
///   0x2,
///     group("VALU") {
///       +group("EXIP") {
///         number("IP", 0x0)
///         number("PORT", 0x0)
///       }
///       +group("INIP") {
///         number("IP", 0x0)
///         number("PORT", 0x0)
///       }
///     }
///   )
///   number("QCAP", 0x0)
///   number("VOIP", 0x2)
/// }
/// ```
async fn handle_start_matchmaking(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<MatchmakingReq>()?;
    {
        let session_data = session.data.read().await;
        let Some(player) = session_data.player.as_ref() else {
            warn!("Client attempted to matchmake while not authenticated. (SID: {})", session.id);
            return session.response_error_empty(packet, ServerError::FailedNoLoginAction).await;
        };
        info!("Player {} started matchmaking", player.display_name);
    }

    let rules = parse_ruleset(req.criteria.rules);

    let games = session.games();

    session
        .response(packet, &MatchmakingRes { id: session.id })
        .await?;

    if games.add_or_queue(session, rules).await {
        debug!("Found matching game")
    }
    Ok(())
}

/// Handles cancelling matchmaking for the current session removing
/// itself from the matchmaking queue.
///
/// # Structure
/// ```
/// packet(Components.GAME_MANAGER, Commands.CANCEL_MATCHMAKING, 0x54) {
///  number("MSID", 0x10d2d0df)
/// }
/// ```
async fn handle_cancel_matchmaking(session: &SessionArc, packet: &Packet) -> HandleResult {
    let session_data = session.data.read().await;
    let Some(player) = session_data.player.as_ref() else {
            warn!("Client attempted to cancel matchmaking while not authenticated. (SID: {})", session.id);
            return session.response_error_empty(packet, ServerError::FailedNoLoginAction).await;
        };
    info!("Player {} cancelled matchmaking", player.display_name);

    session.response_empty(packet).await?;

    session.release_games().await;
    Ok(())
}
