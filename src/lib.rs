#![allow(incomplete_features)]
#![feature(result_flattening)]
#![feature(specialization)]

extern crate combadge_macros;
pub use combadge_macros::combadge;

mod callback;
pub use callback::{AsyncClosure, Callback1};
mod client;
pub use client::Client;
mod error;
pub use error::Error;
mod message;
pub use message::{Message, Post};
mod result;
pub use result::Result;
mod server;
pub use server::Server;

pub mod reexports {
    pub use ::futures;
    pub use ::js_sys;
    pub use ::serde;
    pub use ::wasm_bindgen;
    pub use ::web_sys;
}
