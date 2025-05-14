#![feature(min_specialization)]

use combadge::prelude::*;

mod error;
pub use error::Error;
use js_sys::Uint32Array;

pub type MessageCallback = Callback<(String,), Result<(), Error>>;

#[derive(Post)]
pub struct Postable {
    pub data: Uint32Array,
}

impl From<Uint32Array> for Postable {
    fn from(data: Uint32Array) -> Self {
        Self { data }
    }
}

#[derive(Post, Transfer)]
pub struct Transferable {
    pub data: Uint32Array,
}

impl From<Uint32Array> for Transferable {
    fn from(data: Uint32Array) -> Self {
        Self { data }
    }
}

#[combadge]
pub trait Sample {
    fn add(&self, a: f32, b: f32) -> f32;
    fn parse(&self, string: String) -> Result<i32, Error>;
    fn call_with_message(&self, callback: MessageCallback, message: String);
    fn block_thread(&self) -> Result<(), Error>;
    fn double_postable(&self, data: Uint32Array) -> Result<Postable, Error>;
    fn double_transferable(&self, data: Uint32Array) -> Result<Transferable, Error>;
    fn get_future(&self) -> Box<dyn Future<Output = String>>;
}
