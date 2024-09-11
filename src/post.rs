use std::any::type_name;

use js_sys::Array;
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::MessagePort;

#[cfg(feature = "experimental_shared_memory")]
use js_sys::Reflect;
#[cfg(feature = "experimental_shared_memory")]
use wasm_bindgen::convert::RefFromWasmAbi;

use crate::Error;

trait SerdePost: Sized {
    const POSTABLE: bool;
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(&self) -> Result<JsValue, Error>;
}

impl<T> SerdePost for T {
    default const POSTABLE: bool = false;

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
    const POSTABLE: bool = true;

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
    const POSTABLE: bool;
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(self) -> Result<JsValue, Error>;
}

impl<T> WasmPost for T {
    const POSTABLE: bool = <T as SerdePost>::POSTABLE;

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

        #[cfg(feature = "log")]
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

        #[cfg(feature = "log")]
        log::info!("serializing {ptr}");

        Ok(value)
    }
}

trait JsPost: Sized {
    const POSTABLE: bool;
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(self) -> Result<JsValue, Error>;
}

impl<T: Sized> JsPost for T {
    default const POSTABLE: bool = <T as WasmPost>::POSTABLE;

    default fn from_js_value(value: JsValue) -> Result<Self, Error> {
        WasmPost::from_js_value(value)
    }

    default fn to_js_value(self) -> Result<JsValue, Error> {
        WasmPost::to_js_value(self)
    }
}

impl<T> JsPost for T
where
    T: Into<JsValue> + From<JsValue>,
{
    const POSTABLE: bool = true;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        Ok(value.into())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        Ok(self.into())
    }
}

pub trait Post: Sized {
    const POSTABLE: bool;
    fn from_js_value(value: JsValue) -> Result<Self, Error>;
    fn to_js_value(self) -> Result<JsValue, Error>;
}

impl<T> Post for T
where
    T: Sized,
{
    default const POSTABLE: bool = <T as JsPost>::POSTABLE;

    default fn from_js_value(value: JsValue) -> Result<Self, Error> {
        JsPost::from_js_value(value)
    }

    default fn to_js_value(self) -> Result<JsValue, Error> {
        JsPost::to_js_value(self)
    }
}

impl<T, E> Post for Result<T, E> {
    const POSTABLE: bool = <T as Post>::POSTABLE && <E as Post>::POSTABLE;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let value: Array = value.into();
        let tag = value
            .at(0)
            .as_string()
            .ok_or_else(|| Error::DeserializeFailed {
                type_name: String::from(type_name::<Self>()),
                error: String::from("failed to convert first field to string"),
            })?;

        match tag.as_str() {
            "Ok" => Ok(Self::Ok(Post::from_js_value(value.at(1))?)),
            "Err" => Ok(Self::Err(Post::from_js_value(value.at(1))?)),
            _ => Err(Error::DeserializeFailed {
                type_name: String::from(type_name::<Self>()),
                error: format!("found unexpected tag {tag}"),
            }),
        }
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        match self {
            Self::Ok(value) => {
                Ok(Array::of2(&JsValue::from_str("Ok"), &Post::to_js_value(value)?).into())
            }
            Self::Err(error) => {
                Ok(Array::of2(&JsValue::from_str("Err"), &Post::to_js_value(error)?).into())
            }
        }
    }
}

pub trait Transfer {
    const NEEDS_TRANSFER: bool;
}

impl<T> Transfer for T {
    default const NEEDS_TRANSFER: bool = false;
}

impl Transfer for MessagePort {
    const NEEDS_TRANSFER: bool = true;
}
