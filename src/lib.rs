#![allow(incomplete_features)]
#![feature(result_flattening)]
#![feature(specialization)]

extern crate combadge_macros;

mod callback;
pub use callback::{AsyncClosure, Callback};
mod client;
pub use client::Client;
mod error;
pub use error::Error;
mod handle;
pub use handle::{AsHandle, Handle};
mod message;
pub use message::Message;
mod port;
pub use port::Port;
mod post;
pub use post::{Post, Transfer};
mod server;
pub use server::Server;
mod to_async;
pub use to_async::{MaybeAsync, ToAsync};

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
    pub use combadge_macros::{combadge, proxy};
}
