//! Support for Tendermint RPC version 0.34.
//!
//! The API in this module provides compatibility with the Tendermint RPC
//! protocol as implemented in [Tendermint Core][tendermint] version 0.34.
//!
//! [tendermint]: https://github.com/tendermint/tendermint

mod client;
pub mod endpoint;
pub mod event;
mod serializers;

#[cfg(feature = "websocket-client")]
pub use client::WebSocketClient;
pub use client::{Subscription, SubscriptionClient};
