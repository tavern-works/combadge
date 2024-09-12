use std::any::type_name;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{MessageChannel, MessagePort};

use crate::{Error, Post, Transfer};

pub trait AsHandle<T> {
    type Client;
    type Server;
    fn into_client(port: MessagePort) -> Self::Client;
    fn create_server(local: T, port: MessagePort);
}

pub struct Handle<T: AsHandle<T>> {
    local: Option<T>,
    remote: Option<MessagePort>,
}

impl<T: AsHandle<T>> From<T> for Handle<T> {
    fn from(local: T) -> Self {
        Self {
            local: Some(local),
            remote: None,
        }
    }
}

impl<T: AsHandle<T>> Handle<T> {
    pub fn new_remote(port: MessagePort) -> Self {
        Self {
            local: None,
            remote: Some(port),
        }
    }

    pub fn try_into_client(self) -> Result<T::Client, Error> {
        self.remote
            .ok_or_else(|| Error::CreationFailed {
                type_name: String::from(type_name::<T::Client>()),
                error: String::from("tried to create a proxy from a handle without a remote port"),
            })
            .map(|port| T::into_client(port))
    }
}

impl<T: AsHandle<T>> Post for Handle<T> {
    const POSTABLE: bool = false;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let port: MessagePort = value.dyn_into().map_err(|error| Error::DeserializeFailed {
            type_name: String::from(type_name::<T>()),
            error: format!("failed to convert JsValue to MessagePort for Handle: {error:?}"),
        })?;
        Ok(Handle::new_remote(port))
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        let Some(local) = self.local else {
            return Err(Error::SerializeFailed {
                type_name: String::from(type_name::<T>()),
                error: String::from("local not set when trying to send"),
            });
        };

        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("failed to create MessageChannel in Handle::to_js_value: {error:?}"),
        })?;

        T::create_server(local, channel.port1());
        Post::to_js_value(channel.port2())
    }
}

impl<T: AsHandle<T>> Transfer for Handle<T> {
    const NEEDS_TRANSFER: bool = true;
}
