use std::{
    f32::consts::E,
    future::ready,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{Method, StatusCode},
};
use hyper::upgrade::{OnUpgrade, Upgraded};
use std::io;

pub struct BlazeUpgrade {
    socket_addr: SocketAddr,
    on_upgrade: OnUpgrade,
}

#[derive(Debug)]
pub enum BlazeUpgradeError {
    FailedUpgrade,
}

impl BlazeUpgrade {
    pub async fn upgrade(self) -> Result<BlazeSocket, BlazeUpgradeError> {
        let upgrade = match self.on_upgrade.await {
            Ok(value) => value,
            Err(_) => return Err(BlazeUpgradeError::FailedUpgrade),
        };

        Ok(BlazeSocket {
            upgrade,
            socket_addr: self.socket_addr,
        })
    }
}

impl<S> FromRequestParts<S> for BlazeUpgrade
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut axum::http::request::Parts,
        state: &'life1 S,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<Self, Self::Rejection>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        // let remote_addr = parts
        //     .extensions
        //     .get::<ConnectInfo<SocketAddr>>()
        //     .map(|ci| ci.0)
        //     .unwrap();

        Box::pin(async move {
            if parts.method != Method::GET {
                return Err(StatusCode::BAD_REQUEST);
            }

            println!("WAIT UPGRADE");

            let on_upgrade = parts
                .extensions
                .remove::<OnUpgrade>()
                .ok_or(StatusCode::BAD_REQUEST)?;

            println!("UPGRADING");

            Ok(Self {
                on_upgrade,
                socket_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            })
        })
    }
}

pub struct BlazeSocket {
    pub upgrade: Upgraded,
    pub socket_addr: SocketAddr,
}
