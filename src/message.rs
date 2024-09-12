use combadge_macros::build_post_tuple;
use js_sys::Array;
use wasm_bindgen::prelude::*;

#[cfg(feature = "experimental_shared_memory")]
use js_sys::Reflect;
#[cfg(feature = "experimental_shared_memory")]
use wasm_bindgen::convert::RefFromWasmAbi;

use crate::{Error, Post, Transfer};

pub struct Message {
    message: Vec<JsValue>,
    transfer: Vec<JsValue>,
}

impl Message {
    pub fn new(name: &str) -> Self {
        Self {
            message: vec![JsValue::from_str(name)],
            transfer: Vec::new(),
        }
    }

    pub fn post<T>(&mut self, message: T) -> Result<(), Error>
    where
        T: Post + Transfer,
    {
        let post = message.to_js_value()?;
        self.message.push(post.clone());
        if T::NEEDS_TRANSFER {
            self.transfer.push(post)
        }
        Ok(())
    }

    pub fn send<T>(self, sender: T) -> Result<(), Error>
    where
        T: FnOnce(&JsValue, &JsValue) -> Result<(), Error>,
    {
        let message = self.message.into_iter().collect::<Array>();
        let transfer = self.transfer.into_iter().collect::<Array>();
        sender(message.as_ref(), transfer.as_ref())
    }
}

pub(crate) trait PostTuple<T> {
    fn post_tuple(&mut self, tuple: T) -> Result<(), Error>;
}

build_post_tuple!(7);
