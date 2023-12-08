//! Clients connect through this service in order to form a connection
//! with a host player without the possible NAT restrictions that may
//! occur on stricter NATs
//!
//!
//!
//!
//! Client(s) -- Sends packets to -> Local host socket
//!
//! Local host socket -- Sends packet with index 0 --> Server
//!
//! Server -- Forwards packets to --> Host local pool
//!
//! Host local pool -- Sends packet pretending to be the other client --> Host
//!
//! Host -- Sends reply to --> Host local pool
//!
//! Host local pool -- Sends reply with index --> Server
//!
//! Server -- Forwards packets to index --> Client
//!

use std::{
    io,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicU32, AtomicUsize},
        Arc,
    },
};

use bytes::{Buf, BufMut, Bytes};
use futures_util::{SinkExt, StreamExt};
use hashbrown::HashMap;
use hyper::upgrade::Upgraded;
use log::debug;
use parking_lot::Mutex;
use tokio::{select, sync::mpsc};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::utils::types::GameID;

pub struct TunnelService {
    /// Mapping between host addreses and access to their tunnel
    pub tunnels: Mutex<HashMap<Ipv4Addr, TunnelHandle>>,
    /// Tunnel pooling allocated for games
    pub pools: Mutex<HashMap<GameID, Arc<Mutex<TunnelPool>>>>,
    /// Mapping for which game a tunnel is connected to
    pub mapping: Mutex<HashMap<u32, GameID>>,
}

impl TunnelService {
    pub fn new() -> Self {
        Self {
            tunnels: Mutex::new(Default::default()),
            pools: Mutex::new(Default::default()),
            mapping: Mutex::new(Default::default()),
        }
    }
}

static TUNNEL_ID: AtomicU32 = AtomicU32::new(1);

/// Represents a pool
pub struct TunnelPool {
    pub handles: [Option<TunnelHandle>; 4],
}

/// Handle for sending messages to a tunnel
#[derive(Clone)]
pub struct TunnelHandle(mpsc::UnboundedSender<TunnelMessage>);

pub struct Tunnel {
    service: Arc<TunnelService>,
    id: u32,
    /// The IO tunnel used to send information to the host and recieve
    /// respones
    io: Framed<Upgraded, TunnelCodec>,
    /// Reciever for messages that should be written to the tunnel
    rx: mpsc::UnboundedReceiver<TunnelMessage>,
}

impl Tunnel {
    pub fn start(service: Arc<TunnelService>, io: Framed<Upgraded, TunnelCodec>) -> TunnelHandle {
        let (tx, rx) = mpsc::unbounded_channel();

        let id = TUNNEL_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let tunnel = Tunnel {
            service,
            id,
            io,
            rx,
        };

        tokio::spawn(async move {
            tunnel.handle().await;
        });

        TunnelHandle(tx)
    }

    pub async fn handle(mut self) {
        loop {
            select! {
                        message = self.io.next() => {
                            if let Some(Ok(message)) = message {

                            let index =message.index as usize;
            debug!("Message for {index}");
                            if let Some(pool) = self.service.get_pool_for(self.id) {
                                let pool = &mut *pool.lock();
                                if let Some(Some(handle)) = pool.handles.get(index) {
                                    handle.0.send(message).unwrap();
                                }
                            }
                        } else {
                            debug!("Dropping tunnel");
                            break;
                        }
                        }

                        message = self.rx.recv() => {
                            if let Some(message) = message {
                                debug!("Outgoing message");
                            self.io.send(message).await.unwrap();
                            }
                        }
                    }
        }
    }
}

impl TunnelService {
    pub fn get_pool_for(&self, tunnel_id: u32) -> Option<Arc<Mutex<TunnelPool>>> {
        let game_id = *self.mapping.lock().get(&tunnel_id)?;
        self.pools.lock().get(&game_id).cloned()
    }

    /// Gets the tunnel for the provided IP address if one is present
    pub fn get_tunnel(&self, addr: Ipv4Addr) -> Option<TunnelHandle> {
        let tunnels = &*self.tunnels.lock();
        tunnels.get(&addr).cloned()
    }

    pub fn set_tunnel(&self, addr: Ipv4Addr, tunnel: TunnelHandle) {
        let tunnels = &mut *self.tunnels.lock();
        tunnels.insert(addr, tunnel);
    }

    pub fn remove_tunnel(&self, addr: Ipv4Addr) {
        let tunnels = &mut *self.tunnels.lock();
        tunnels.remove(&addr);
    }

    /// Sets the handle at the provided index within a pool to the provided handle
    pub fn set_pool_handle(&self, game_id: GameID, index: usize, handle: TunnelHandle) {
        let pools = &mut *self.pools.lock();

        // Get the existing pool or insert a new one
        let pool = pools.entry(game_id).or_insert_with(|| {
            let handles = [None, None, None, None];
            Arc::new(Mutex::new(TunnelPool { handles }))
        });

        let pool = &mut *pool.lock();
        if let Some(pool_handle) = pool.handles.get_mut(index) {
            *pool_handle = Some(handle);
        }
    }
}

/// Partially decoded tunnnel message
pub struct TunnelMessagePartial {
    pub index: u8,
    pub length: u32,
}

/// Message sent through the tunnel
pub struct TunnelMessage {
    /// Socket index to use
    pub index: u8,

    /// The message contents
    pub message: Bytes,
}

#[derive(Default)]
pub struct TunnelCodec {
    partial: Option<TunnelMessagePartial>,
}

impl Decoder for TunnelCodec {
    type Item = TunnelMessage;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let partial = match self.partial.as_mut() {
            Some(value) => value,
            None => {
                // Not enough room for a partial frame
                if src.len() < 5 {
                    return Ok(None);
                }
                let index = src.get_u8();
                let length = src.get_u32();

                self.partial.insert(TunnelMessagePartial { index, length })
            }
        };
        // Not enough data for the partial frame
        if src.len() < partial.length as usize {
            return Ok(None);
        }

        let partial = self.partial.take().expect("Partial frame missing");
        let bytes = src.split_to(partial.length as usize);

        Ok(Some(TunnelMessage {
            index: partial.index,
            message: bytes.freeze(),
        }))
    }
}

impl Encoder<TunnelMessage> for TunnelCodec {
    type Error = io::Error;

    fn encode(
        &mut self,
        item: TunnelMessage,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        dst.put_u8(item.index);
        dst.put_u32(item.message.len() as u32);
        dst.extend_from_slice(&item.message);
        Ok(())
    }
}
