#![deny(unsafe_code)]
// Rustc lint groups
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]
// Rustc lints
#![warn(noop_method_call)]
#![warn(single_use_lifetimes)]
#![warn(trivial_numeric_casts)]
#![warn(unused_crate_dependencies)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
// Clippy lints
#![warn(clippy::use_self)]

pub mod api;
pub mod conn;
mod huehash;
pub mod replies;

pub use huehash::nick_hue;
