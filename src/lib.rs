#![allow(incomplete_features)]
#![feature(specialization)]

extern crate combadge_macros;

mod client;
pub use client::Client;
mod error;
pub use error::Error;
mod message;
pub use message::Message;
mod server;
pub use server::Server;

pub mod prelude {
    pub use ::web_sys;
    pub use combadge_macros::combadge;
}
