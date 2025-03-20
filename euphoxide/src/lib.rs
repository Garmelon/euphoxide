//! API bindings for [euphoria.leet.nu](https://euphoria.leet.nu/).
//!
//! If you want to write a generic client, see the `euphoxide-client` crate for
//! connection management. If you want to write a bot, the `euphoxide-bot` crate
//! contains additional useful tools.
//!
//! # Useful links
//!
//! - the [euphoria.leet.nu API docs](https://euphoria.leet.nu/heim/api)
//! - [heim](https://github.com/CylonicRaider/heim), the software underlying euphoria

pub mod api;
pub mod client;
pub mod conn;
mod emoji;
mod error;
pub mod nick;
mod replies;

pub use crate::{emoji::*, error::*};
