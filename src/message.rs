use std::any::type_name;

use js_sys::Array;
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::JsValue;

use crate::Error;

trait SerdePost: Sized {
    fn deserialize_from_js_value(value: JsValue) -> Result<Self, Error>;
    fn serialize_to_js_value(&self) -> Result<JsValue, Error>;
}

impl<T> SerdePost for T {
    default fn deserialize_from_js_value(_value: JsValue) -> Result<Self, Error> {
        unreachable!()
    }

    default fn serialize_to_js_value(&self) -> Result<JsValue, Error> {
        unreachable!()
    }
}

impl<'de, T> SerdePost for T
where
    T: DeserializeOwned + Serialize,
{
    fn deserialize_from_js_value(value: JsValue) -> Result<Self, Error> {
        serde_wasm_bindgen::from_value(value).map_err(|error| Error::DeserializeFailed {
            type_name: String::from(type_name::<T>()),
            error: format!("{error}"),
        })
    }

    fn serialize_to_js_value(&self) -> Result<JsValue, Error> {
        serde_wasm_bindgen::to_value(self).map_err(|error| Error::SerializeFailed {
            type_name: String::from(type_name::<T>()),
            error: format!("{error}"),
        })
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
        T::deserialize_from_js_value(value)
    }

    default fn to_js_value(self) -> Result<JsValue, Error> {
        SerdePost::serialize_to_js_value(&self)
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

    pub fn transfer<T>(&mut self, message: T)
    where
        T: Post + Clone,
    {
        let post = message.to_js_value().unwrap();
        self.message.push(post.clone());
        self.transfer.push(post);
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
