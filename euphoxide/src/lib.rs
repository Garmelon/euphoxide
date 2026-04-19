//! API bindings for [euphoria.leet.nu](https://euphoria.leet.nu/).
//!
//! If you want to write a generic client, see the `euphoxide-client` crate for
//! connection management. If you want to write a bot, the command framework in
//! the `euphoxide-command` crate may prove useful.
//!
//! See also the [euphoria.leet.nu API docs](https://euphoria.leet.nu/heim/api)
//! and [heim](https://github.com/CylonicRaider/heim), the software underlying euphoria.

// TODO Move to workspace Cargo.toml
#![warn(missing_docs)]

pub mod api;
pub mod client;
pub mod conn;
mod emoji;
mod error;
pub mod nick;
mod replies;

pub use crate::{emoji::*, error::*};
