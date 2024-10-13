use std::{net::Ipv4Addr, sync::Arc, task::Context, time::Duration};

use parking_lot::{RwLock, RwLockReadGuard};
use serde::Serialize;
use tokio::time::{interval_at, Instant, Interval, MissedTickBehavior};

use crate::{
    database::entities::Player,
    services::{
        game::{GameRef, WeakGameRef},
        sessions::{AssociationId, SessionPlayerAssociation},
    },
    utils::{
        components::user_sessions,
        types::{GameID, PlayerID},
    },
};

use super::{
    models::{
        game_manager::RemoveReason,
        user_sessions::{
            HardwareFlags, LookupResponse, NotifyUserAdded, NotifyUserRemoved, NotifyUserUpdated,
            UserDataFlags, UserIdentification, UserSessionExtendedData,
            UserSessionExtendedDataUpdate,
        },
        NetworkAddress, QosNetworkData,
    },
    packet::Packet,
    SessionNotifyHandle,
};

pub struct SessionData {
    /// Extended session data, writable data
    ext: RwLock<SessionDataExt>,

    /// IP address associated with the session
    addr: Ipv4Addr,

    /// User will not have an association if they are using an outdated
    /// client version.
    association: Option<AssociationId>,
}

struct SessionDataExt {
    /// Data for authorized sessions
    auth: Option<SessionDataAuth>,

    /// Keep-alive data for the session
    keep_alive: SessionDataKeepAlive,
}

impl SessionDataExt {
    fn new() -> Self {
        Self {
            auth: None,
            keep_alive: SessionDataKeepAlive::new(),
        }
    }
}

pub struct SessionDataKeepAlive {
    /// Last time a keep-alive message was received through the tunnel
    pub last_keep_alive: Instant,

    /// Time that has been granted as a grace period to allow the
    /// session to go without a keep-alive message for
    pub extended_grace: Duration,

    /// Interval for polling connection alive checks
    pub keep_alive_interval: Interval,
}

/// Delay between each keep-alive check
pub const KEEP_ALIVE_DELAY: Duration = Duration::from_secs(15);

/// When this duration elapses between keep-alive checks for a connection
/// the connection is considered to be dead (4 missed keep-alive check intervals)
pub const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(KEEP_ALIVE_DELAY.as_secs() * 4);

impl SessionDataKeepAlive {
    fn new() -> Self {
        let now = Instant::now();

        // Create the interval to track keep alive checking
        let keep_alive_start = now + KEEP_ALIVE_DELAY;
        let mut keep_alive_interval = interval_at(keep_alive_start, KEEP_ALIVE_DELAY);

        keep_alive_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        Self {
            last_keep_alive: Instant::now(),
            extended_grace: Duration::from_secs(0),
            keep_alive_interval,
        }
    }
}

impl SessionData {
    /// Creates new session data
    pub fn new(addr: Ipv4Addr, association: Option<AssociationId>) -> Self {
        Self {
            ext: RwLock::new(SessionDataExt::new()),
            addr,
            association,
        }
    }

    /// Polls the keep alive check to see if its ready and if the connection is dead
    pub fn poll_keep_alive_dead(&self, cx: &mut Context<'_>) -> bool {
        let keep_alive = &mut self.ext.write().keep_alive;

        // Not ready to perform a keep-alive check
        if !keep_alive.keep_alive_interval.poll_tick(cx).is_ready() {
            return false;
        }

        // Check the keep alive state
        let now = Instant::now();
        let last_alive = keep_alive
            .last_keep_alive
            // Get time since last keep alive message
            .duration_since(now)
            // Remove current grace period from the elapsed time
            .saturating_sub(keep_alive.extended_grace);

        // Connection to the client has timed out as no keep alive messages were
        // given by the client
        last_alive > KEEP_ALIVE_TIMEOUT
    }

    /// Sets the connection as alive
    pub fn set_alive(&self) {
        let keep_alive = &mut self.ext.write().keep_alive;

        // Clear existing grace period
        keep_alive.extended_grace = Duration::from_secs(0);

        // Mark current alive period
        keep_alive.last_keep_alive = Instant::now();
    }

    /// Grants a grace period duration where the client is allowed to not send any keep-alive
    /// messages and won't be timed-out for doing so
    pub fn set_keep_alive_grace(&self, grace: Duration) {
        let keep_alive = &mut self.ext.write().keep_alive;
        let now = Instant::now()
            .checked_add(grace)
            .expect("reached limit of time");

        // Delay next keep alive check
        keep_alive.keep_alive_interval.reset_at(now);

        // Apply grace period to next check
        keep_alive.extended_grace = grace;
    }

    pub fn get_addr(&self) -> Ipv4Addr {
        self.addr
    }

    pub fn get_association(&self) -> Option<AssociationId> {
        self.association
    }

    // Read from the underlying session data
    fn read(&self) -> RwLockReadGuard<'_, SessionDataExt> {
        self.ext.read()
    }

    /// Writes to the underlying session data without publishing the changes
    fn write_silent<F, O>(&self, update: F) -> Option<O>
    where
        F: FnOnce(&mut SessionDataAuth) -> O,
    {
        self.ext.write().auth.as_mut().map(update)
    }

    /// Writes to the underlying session data, publishes changes to
    /// subscribers
    #[inline]
    fn write_publish<F, O>(&self, update: F) -> Option<O>
    where
        F: FnOnce(&mut SessionDataAuth) -> O,
    {
        self.ext.write().auth.as_mut().map(|data| {
            let value = update(data);
            data.publish_update();
            value
        })
    }

    /// Clears the underlying authenticated session data
    pub fn clear_auth(&self) {
        self.ext.write().auth.take();
    }

    /// Starts a session from the provided player association
    pub fn set_auth(&self, player: SessionPlayerAssociation) -> Arc<Player> {
        self.ext
            .write()
            .auth
            .insert(SessionDataAuth::new(player))
            // Obtain the player to return
            .player_assoc
            .player
            .clone()
    }

    /// Gets the currently authenticated player
    pub fn get_player(&self) -> Option<Arc<Player>> {
        self.read()
            .auth
            .as_ref()
            .map(|value| value.player_assoc.player.clone())
    }

    /// Obtains the parts required to create a game player
    pub fn get_game_player_data(&self) -> Option<(Arc<Player>, Arc<NetData>)> {
        self.read()
            .auth
            .as_ref()
            .map(|value| (value.player_assoc.player.clone(), value.net.clone()))
    }

    /// Updates the session hardware flags
    pub fn set_hardware_flags(&self, value: HardwareFlags) {
        self.write_publish(|data| data.net = Arc::new(data.net.with_hardware_flags(value)));
    }

    /// Sets the current session network information
    pub fn set_network_info(
        &self,
        address: NetworkAddress,
        qos: QosNetworkData,
        ping_site_latency: Vec<u32>,
    ) {
        self.write_publish(|data| {
            data.net = Arc::new(data.net.with_basic(address, qos, ping_site_latency))
        });
    }

    /// Obtains the network data for the session
    pub fn network_info(&self) -> Option<Arc<NetData>> {
        self.read().auth.as_ref().map(|value| value.net.clone())
    }

    /// Sets the game the session is currently apart of
    pub fn set_game(&self, game_id: GameID, game_ref: WeakGameRef) {
        // Set the current game
        self.write_publish(|data| {
            data.game = Some(SessionGameData {
                player_id: data.player_assoc.player.id,
                game_id,
                game_ref,
            });
        });
    }

    /// Clears the game the session is apart of
    pub fn clear_game(&self) {
        self.write_publish(|data| data.game = None);
    }

    /// Obtains the ID and reference to the game the session is currently apart of
    pub fn get_game(&self) -> Option<(GameID, GameRef)> {
        let guard = self.read();

        let data = guard.auth.as_ref()?;
        let game_data = data.game.as_ref()?;

        let game_ref = match game_data.game_ref.upgrade() {
            Some(value) => value,
            // We have a dangling game ref, clean it up
            None => {
                // Drop the guard before writing
                drop(guard);

                // Clear the dangling game ref
                self.clear_game();
                return None;
            }
        };

        Some((game_data.game_id, game_ref))
    }

    pub fn get_lookup_response(&self) -> Option<LookupResponse> {
        self.read().auth.as_ref().map(|data| LookupResponse {
            player: data.player_assoc.player.clone(),
            extended_data: data.ext_data(),
        })
    }

    /// Adds a subscriber to the session
    pub fn add_subscriber(&self, player_id: PlayerID, subscriber: SessionNotifyHandle) {
        self.write_silent(|data| data.add_subscriber(player_id, subscriber));
    }

    /// Removes a subscriber from the session
    pub fn remove_subscriber(&self, player_id: PlayerID) {
        self.write_silent(|data| data.remove_subscriber(player_id));
    }
}

/// Extended session data, present when the user is authenticated
struct SessionDataAuth {
    /// Session -> Player association, currently authenticated player
    player_assoc: Arc<SessionPlayerAssociation>,
    /// Networking information for current session
    net: Arc<NetData>,
    /// Currently connected game for the session
    game: Option<SessionGameData>,
    /// Subscribers listening for changes to this session
    subscribers: Vec<SessionSubscription>,
}

impl SessionDataAuth {
    fn new(player: SessionPlayerAssociation) -> Self {
        Self {
            player_assoc: Arc::new(player),
            net: Default::default(),
            game: Default::default(),
            subscribers: Default::default(),
        }
    }

    fn ext_data(&self) -> UserSessionExtendedData {
        UserSessionExtendedData {
            net: self.net.clone(),
            game: self.game.as_ref().map(|game| game.game_id),
        }
    }

    /// Adds a new subscriber to this session `player_id` is the ID of the player who is
    /// subscribing and `notify_handle` is the handle for sending messages to them
    fn add_subscriber(&mut self, player_id: PlayerID, notify_handle: SessionNotifyHandle) {
        let target_id = self.player_assoc.player.id;

        // Notify the addition of this user data to the subscriber
        notify_handle.notify(Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_ADDED,
            NotifyUserAdded {
                session_data: self.ext_data(),
                user: UserIdentification::from_player(&self.player_assoc.player),
            },
        ));

        // Notify the user that they are now subscribed to this user
        notify_handle.notify(Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_UPDATED,
            NotifyUserUpdated {
                flags: UserDataFlags::SUBSCRIBED | UserDataFlags::ONLINE,
                player_id: target_id,
            },
        ));

        // Add the subscriber
        self.subscribers.push(SessionSubscription {
            target_id,
            source_id: player_id,
            source_notify_handle: notify_handle,
        });
    }

    fn remove_subscriber(&mut self, player_id: PlayerID) {
        self.subscribers
            .retain(|value| value.source_id != player_id);
    }

    /// Publishes changes of the session data to all the
    /// subscribed session links
    fn publish_update(&self) {
        let packet = Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_SESSION_EXTENDED_DATA_UPDATE,
            UserSessionExtendedDataUpdate {
                user_id: self.player_assoc.player.id,
                data: self.ext_data(),
            },
        );

        self.subscribers
            .iter()
            .for_each(|sub| sub.source_notify_handle.notify(packet.clone()));
    }
}

/// Subscription to a session to be notified when the session details
/// change.
struct SessionSubscription {
    /// ID of the player being subscribed to
    target_id: PlayerID,
    /// ID of the player who is subscribing
    source_id: PlayerID,
    /// Handle to send messages to the source
    source_notify_handle: SessionNotifyHandle,
}

impl Drop for SessionSubscription {
    fn drop(&mut self) {
        // Notify the subscriber they've removed the user subscription
        self.source_notify_handle.notify(Packet::notify(
            user_sessions::COMPONENT,
            user_sessions::USER_REMOVED,
            NotifyUserRemoved {
                player_id: self.target_id,
            },
        ))
    }
}

/// When dropped if the player is still connected to the game they will
/// be disconnected from the game
struct SessionGameData {
    /// ID of the player session when they joined the game
    player_id: PlayerID,
    /// ID of the game that was joined
    game_id: GameID,
    /// Reference for accessing the game
    game_ref: WeakGameRef,
}

impl Drop for SessionGameData {
    fn drop(&mut self) {
        // Attempt to access the game
        let game_ref = match self.game_ref.upgrade() {
            Some(value) => value,
            // Game doesn't exist anymore
            None => return,
        };

        let player_id = self.player_id;

        // Spawn an async task to handle removing the player
        tokio::spawn(async move {
            let game = &mut *game_ref.write().await;
            game.remove_player(player_id, RemoveReason::PlayerLeft);
        });
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub addr: NetworkAddress,
    pub qos: QosNetworkData,
    pub hardware_flags: HardwareFlags,
    pub ping_site_latency: Vec<u32>,
}

impl NetData {
    // Re-creates the current net data using the provided address and QOS data
    pub fn with_basic(
        &self,
        addr: NetworkAddress,
        qos: QosNetworkData,
        ping_site_latency: Vec<u32>,
    ) -> Self {
        Self {
            addr,
            qos,
            hardware_flags: self.hardware_flags,
            ping_site_latency,
        }
    }

    /// Re-creates the current net data using the provided hardware flags
    pub fn with_hardware_flags(&self, flags: HardwareFlags) -> Self {
        Self {
            addr: self.addr.clone(),
            qos: self.qos,
            hardware_flags: flags,
            ping_site_latency: self.ping_site_latency.clone(),
        }
    }
}
