use std::any::type_name;

use js_sys::Array;
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[cfg(feature = "experimental_shared_memory")]
use js_sys::Reflect;
#[cfg(feature = "experimental_shared_memory")]
use wasm_bindgen::convert::RefFromWasmAbi;

use crate::Error;

trait SerdePost: Sized {
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(&self) -> Result<JsValue, Error>;
}

impl<T> SerdePost for T {
    default fn from_js_value(_value: JsValue) -> Result<Self, Error> {
        Err(Error::UnsupportedType {
            name: String::from(type_name::<T>()),
        })
    }

    default fn to_js_value(&self) -> Result<JsValue, Error> {
        Err(Error::UnsupportedType {
            name: String::from(type_name::<T>()),
        })
    }
}

impl<T> SerdePost for T
where
    T: DeserializeOwned + Serialize,
{
    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        serde_wasm_bindgen::from_value(value).map_err(|error| {
            #[cfg(feature = "log")]
            log::error!("error after trying to deserialize {error:?}");
            Error::DeserializeFailed {
                type_name: String::from(type_name::<T>()),
                error: format!("{error:?}"),
            }
        })
    }

    fn to_js_value(&self) -> Result<JsValue, Error> {
        let serializer = serde_wasm_bindgen::Serializer::json_compatible()
            .serialize_large_number_types_as_bigints(true);
        self.serialize(&serializer)
            .map_err(|error| Error::SerializeFailed {
                type_name: String::from(type_name::<T>()),
                error: format!("{error:?}"),
            })
    }
}

pub trait WasmPost: Sized {
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(self) -> Result<JsValue, Error>;
}

impl<T> WasmPost for T {
    default fn from_js_value(value: JsValue) -> Result<Self, Error> {
        SerdePost::from_js_value(value)
    }

    default fn to_js_value(self) -> Result<JsValue, Error> {
        SerdePost::to_js_value(&self)
    }
}

// If both ends of the communication share the same WASM memory, it should be
// possible to extract the pointer from the JsValue and use it on both sides of
// the channel. However, this has not been tested, so try it at your own risk.
#[cfg(feature = "experimental_shared_memory")]
impl<T> WasmPost for T
where
    T: Into<JsValue> + RefFromWasmAbi<Abi = u32> + Clone + std::fmt::Debug,
{
    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let ptr = Reflect::get(&value, &JsValue::from_str("__wbg_ptr"))
            .map_err(|error| Error::DeserializeFailed {
                type_name: String::from(type_name::<T>()),
                error: format!("__wbg_ptr not found in JsValue: {error:?}"),
            })?
            .as_f64()
            .ok_or_else(|| Error::DeserializeFailed {
                type_name: String::from(type_name::<T>()),
                error: String::from("failed to convert __wbg_ptr to f64"),
            })? as u32;

        let instance_ref = unsafe { T::ref_from_abi(ptr) };
        let cloned = instance_ref.clone();

        log::info!("got {cloned:?}");

        Ok(instance_ref.clone())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        let value = self.into();

        let ptr = Reflect::get(&value, &JsValue::from_str("__wbg_ptr"))
            .map_err(|error| Error::DeserializeFailed {
                type_name: String::from(type_name::<T>()),
                error: format!("__wbg_ptr not found in JsValue: {error:?}"),
            })?
            .as_f64()
            .ok_or_else(|| Error::DeserializeFailed {
                type_name: String::from(type_name::<T>()),
                error: String::from("failed to convert __wbg_ptr to f64"),
            })? as u32;

        log::info!("serializing {ptr}");

        Ok(value)
    }
}

pub trait Post: Sized {
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(self) -> Result<JsValue, Error>;
}

impl<T> Post for T
where
    T: Sized,
{
    default fn from_js_value(value: JsValue) -> Result<Self, Error> {
        WasmPost::from_js_value(value)
    }

    default fn to_js_value(self) -> Result<JsValue, Error> {
        WasmPost::to_js_value(self)
    }
}

impl<T> Post for T
where
    T: Into<JsValue> + From<JsValue>,
{
    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        Ok(value.into())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        Ok(self.into())
    }
}

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
        T: Post,
    {
        self.message.push(message.to_js_value()?);
        Ok(())
    }

    pub fn transfer<T>(&mut self, message: T) -> Result<(), Error>
    where
        T: Post + Clone,
    {
        let post = message.to_js_value()?;
        self.message.push(post.clone());
        self.transfer.push(post);
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
