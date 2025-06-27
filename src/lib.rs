#![allow(
    clippy::future_not_send,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
#![allow(incomplete_features)]
#![feature(specialization)]

extern crate combadge_macros;

mod callback;
pub use callback::Callback;
mod client;
pub use client::Client;
mod error;
pub use error::Error;
mod handle;
pub use handle::{AsHandle, Handle};
mod log;
mod message;
pub use message::Message;
mod port;
pub use port::Port;
mod post;
pub use post::{Post, Transfer};
mod server;
pub use server::Server;
mod maybe_async;
pub use maybe_async::MaybeAsync;

pub mod reexports {
    pub use ::futures;
    pub use ::js_sys;
    pub use ::wasm_bindgen;
    pub use ::wasm_bindgen_futures;
    pub use ::web_sys;
}

pub mod prelude {
    pub use crate::callback::call_traits::*;
    pub use crate::callback::Callback;
    pub use crate::handle::Handle;
    pub use crate::post::{Post, Transfer};
    pub use ::wasm_bindgen::JsCast as _;
    pub use combadge_macros::{combadge, proxy, Post, Transfer};
}
