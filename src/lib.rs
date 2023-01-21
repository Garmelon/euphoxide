#![forbid(unsafe_code)]
// Rustc lint groups
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]
#![warn(unused)]
// Rustc lints
#![warn(noop_method_call)]
#![warn(single_use_lifetimes)]
// Clippy lints
#![warn(clippy::use_self)]

pub mod api;
#[cfg(feature = "bot")]
pub mod bot;
pub mod conn;
mod emoji;
mod huehash;
mod replies;

pub use emoji::Emoji;
pub use huehash::{nick_hue, nick_hue_without_removing_emoji};
