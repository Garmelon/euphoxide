//! Connection management tools for `euphoxide`.
//!
//! A [heim](https://github.com/CylonicRaider/heim) client may want to connect
//! to multiple rooms at the same time, or it may want to reconnect
//! automatically when connection is lost. While the `euphoxide` crate provides
//! API bindings and state management for a single connection to the server,
//! this crate provides tools for managing multiple connections.
//!
//! To get started, create a [`Client`] or [`Clients`].

// TODO Move to workspace Cargo.toml
#![warn(missing_docs)]

mod builder;
mod client;
mod clients;
mod config;

pub use self::{builder::*, client::*, clients::*, config::*};
