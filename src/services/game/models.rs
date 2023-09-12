use super::Game;
use crate::session::models::NetworkAddress;
use tdf::{TdfSerialize, TdfType};

const VSTR: &str = "ME3-295976325-179181965240128";

pub enum GameSetupContext {
    /// Context without additional data
    Dataless(DatalessContext),
    /// Context added from matchmaking
    Matchmaking(u32),
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum DatalessContext {
    /// Session created the game
    CreateGameSetup = 0x0,
    /// Session joined by ID
    JoinGameSetup = 0x1,
    // IndirectJoinGameFromQueueSetup = 0x2,
    // IndirectJoinGameFromReservationContext = 0x3,
    // HostInjectionSetupContext = 0x4,
}

pub struct GameDetails<'a> {
    pub game: &'a Game,
    pub context: GameSetupContext,
}

impl TdfSerialize for GameDetails<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let game = self.game;
        let host_player = match game.players.first() {
            Some(value) => value,
            None => return,
        };

        // Game details
        w.group(b"GAME", |w| {
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            w.tag_ref(b"ATTR", &game.attributes);

            w.tag_list_slice(b"CAP", &[4u8, 0u8]);

            w.tag_u32(b"GID", game.id);
            w.tag_str(b"GNAM", &host_player.player.display_name);

            w.tag_u64(b"GPVH", 0x5a4f2b378b715c6);
            w.tag_u16(b"GSET", game.setting.bits());
            w.tag_u64(b"GSID", 0x4000000a76b645);
            w.tag_ref(b"GSTA", &game.state);

            w.tag_str_empty(b"GTYP");
            {
                w.tag_list_start(b"HNET", TdfType::Group, 1);
                w.write_byte(2);
                if let NetworkAddress::AddressPair(pair) = &host_player.net.addr {
                    TdfSerialize::serialize(pair, w)
                }
            }

            w.tag_u32(b"HSES", host_player.player.id);
            w.tag_zero(b"IGNO");
            w.tag_u8(b"MCAP", 4);
            w.tag_ref(b"NQOS", &host_player.net.qos);
            w.tag_zero(b"NRES");
            w.tag_zero(b"NTOP");
            w.tag_str_empty(b"PGID");
            w.tag_blob_empty(b"PGSR");

            w.group(b"PHST", |w| {
                w.tag_u32(b"HPID", host_player.player.id);
                w.tag_zero(b"HSLT");
            });

            w.tag_u8(b"PRES", 0x1);
            w.tag_str_empty(b"PSAS");
            w.tag_u8(b"QCAP", 0x0);
            w.tag_u32(b"SEED", 0x4cbc8585);
            w.tag_u8(b"TCAP", 0x0);

            w.group(b"THST", |w| {
                w.tag_u32(b"HPID", host_player.player.id);
                w.tag_u8(b"HSLT", 0x0);
            });

            w.tag_str(b"UUID", "286a2373-3e6e-46b9-8294-3ef05e479503");
            w.tag_u8(b"VOIP", 0x2);
            w.tag_str(b"VSTR", VSTR);
            w.tag_blob_empty(b"XNNC");
            w.tag_blob_empty(b"XSES");
        });

        // Player list
        w.tag_list_start(b"PROS", TdfType::Group, game.players.len());
        for (slot, player) in game.players.iter().enumerate() {
            player.encode(game.id, slot, w);
        }

        match &self.context {
            GameSetupContext::Dataless(context) => {
                w.tag_union_start(b"REAS", 0x0);
                w.group(b"VALU", |writer| {
                    writer.tag_u8(b"DCTX", (*context) as u8);
                });
            }
            GameSetupContext::Matchmaking(id) => {
                w.tag_union_start(b"REAS", 0x3);
                w.group(b"VALU", |writer| {
                    const FIT: u16 = 21600;

                    writer.tag_u16(b"FIT", FIT);
                    writer.tag_u16(b"MAXF", FIT);
                    writer.tag_u32(b"MSID", *id);
                    // TODO: Matchmaking result
                    // SUCCESS_CREATED_GAME = 0
                    // SUCCESS_JOINED_NEW_GAME = 1
                    // SUCCESS_JOINED_EXISTING_GAME = 2
                    // SESSION_TIMED_OUT = 3
                    // SESSION_CANCELED = 4
                    // SESSION_TERMINATED = 5
                    // SESSION_ERROR_GAME_SETUP_FAILED = 6
                    writer.tag_u8(b"RSLT", 0x2);
                    writer.tag_u32(b"USID", *id);
                });
            }
        }
    }
}

pub struct GetGameDetails<'a> {
    pub game: &'a Game,
}

impl TdfSerialize for GetGameDetails<'_> {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        let game = self.game;
        let host_player = match game.players.first() {
            Some(value) => value,
            None => return,
        };

        w.tag_list_start(b"GDAT", TdfType::Group, 1);
        w.group_body(|w| {
            w.tag_list_iter_owned(b"ADMN", game.players.iter().map(|player| player.player.id));
            w.tag_ref(b"ATTR", &game.attributes);
            w.tag_list_slice(b"CAP", &[4u8, 0u8]);

            w.tag_u32(b"GID", game.id);
            w.tag_str(b"GNAM", &host_player.player.display_name);
            w.tag_u16(b"GSET", game.setting.bits());
            w.tag_ref(b"GSTA", &game.state);
            {
                w.tag_list_start(b"HNET", TdfType::Group, 1);
                w.write_byte(2);
                if let NetworkAddress::AddressPair(pair) = &host_player.net.addr {
                    TdfSerialize::serialize(pair, w)
                }
            }
            w.tag_u32(b"HOST", host_player.player.id);
            w.tag_zero(b"NTOP");

            w.tag_list_slice(b"PCNT", &[1u8, 0u8]);

            w.tag_u8(b"PRES", 0x2);
            w.tag_str(b"PSAS", "ea-sjc");
            w.tag_str_empty(b"PSID");
            w.tag_zero(b"QCAP");
            w.tag_zero(b"QCNT");
            w.tag_zero(b"SID");
            w.tag_zero(b"TCAP");
            w.tag_u8(b"VOIP", 0x2);
            w.tag_str(b"VSTR", VSTR);
        });
    }
}
