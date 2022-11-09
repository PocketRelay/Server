use crate::blaze::components::{AssociationLists, Components, GameReporting};
use crate::blaze::errors::HandleResult;
use crate::blaze::SessionArc;
use blaze_pk::{
    tag_group_end, tag_group_start, tag_list_start, tag_str, tag_triple, tag_u8,
    tag_var_int_list_empty, tag_zero, Codec, OpaquePacket, ValueType,
};
use log::debug;

/// Routing function for handling packets with the `GameReporting` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route_game_reporting(
    session: &SessionArc,
    component: GameReporting,
    packet: &OpaquePacket,
) -> HandleResult {
    match component {
        GameReporting::SubmitOfflineGameReport => handle_submit_offline(session, packet).await,
        component => {
            debug!("Got GameReporting({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}

/// Handles submission of offline game reports from clients.
///
/// # Structure
/// ```
/// packet(Components.GAME_REPORTING, Commands.SUBMIT_OFFLINE_GAME_REPORT, 0x85) {
///   number("FNSH", 0x0)
///   varList("PRVT", listOf())
///   +group("RPRT") {
///     varList("GAME", listOf(0xc63cbd07))
///     +group("GAME") {
///       +group("GAME") {
///       }
///       map("PLYR", mapOf(
///         0x3a5508eb to         group {
///           number("CTRY", 0x4155)
///           number("NCHP", 0x0)
///           number("NRAT", 0x1)
///         },
///       ))
///     }
///   }
///   number("GRID", 0x0)
///   text("GTYP", "massEffectReport")
/// }
/// ```
async fn handle_submit_offline(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    session.response_empty(packet).await?;
    session
        .notify_immediate(
            Components::GameReporting(GameReporting::NotifyGameReportSubmitted),
            &GameReportResult,
        )
        .await?;
    Ok(())
}

#[derive(Debug)]
struct GameReportResult;

impl Codec for GameReportResult {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_var_int_list_empty(output, "DATA");
        tag_zero(output, "EROR");
        tag_zero(output, "FNL");
        tag_zero(output, "GHID");
        tag_zero(output, "GRID");
    }
}

/// Routing function for handling packets with the `AssociationLists` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route_association_lists(
    session: &SessionArc,
    component: AssociationLists,
    packet: &OpaquePacket,
) -> HandleResult {
    match component {
        AssociationLists::GetLists => handle_get_lists(session, packet).await,
        component => {
            debug!("Got AssociationLists({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}

struct DefaultAssocList;

impl Codec for DefaultAssocList {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_list_start(output, "LMAP", ValueType::Group, 1);
        {
            tag_group_start(output, "INFO");

            tag_triple(output, "BOID", &(0x19, 0x1, 0x74b09c4));
            tag_u8(output, "FLGS", 4);

            {
                tag_group_start(output, "LID");
                tag_str(output, "LNM", "friendList");
                tag_u8(output, "TYPE", 1);
                tag_group_end(output);
            }

            tag_u8(output, "LMS", 0xC8);
            tag_u8(output, "PRID", 0);

            tag_group_end(output);
        }
        tag_u8(output, "OFRC", 0);
        tag_u8(output, "TOCT", 0);
        tag_group_end(output);
    }
}

/// Handles getting associated lists for the player
///
/// # Structure
/// ```
/// packet(Components.ASSOCIATION_LISTS, Commands.GET_LISTS, 0x0, 0x21) {
///   list("ALST", listOf(
///     group {
///       tripple("BOID", 0x0, 0x0, 0x0)
///       number("FLGS", 0x1)
///       +group("LID") {
///         text("LNM", "")
///         number("TYPE", 0x1)
///       }
///       number("LMS", 0x0)
///       number("PRID", 0x0)
///     }
///   ))
///   number("MXRC", 0xffffffff)
///   number("OFRC", 0x0)
/// }
/// ```
async fn handle_get_lists(session: &SessionArc, packet: &OpaquePacket) -> HandleResult {
    session.response(packet, &DefaultAssocList).await
}
