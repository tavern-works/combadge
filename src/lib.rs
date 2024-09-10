#![allow(incomplete_features)]
#![feature(specialization)]

extern crate combadge_macros;

mod client;
pub use client::Client;
mod error;
pub use error::Error;
mod message;
pub use message::{Message, Post};
mod server;
pub use server::Server;

pub mod prelude {
    pub use ::js_sys;
    pub use ::serde;
    pub use ::static_assertions;
    pub use ::wasm_bindgen;
    pub use ::web_sys;
    pub use combadge_macros::combadge;
}
