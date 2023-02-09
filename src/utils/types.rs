use std::{future::Future, pin::Pin};

/// Types for differentiating between fields
pub type PlayerID = u32;
pub type SessionID = u32;
pub type GameID = u32;
pub type GameSlot = usize;

/// Type for boxed futures
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
