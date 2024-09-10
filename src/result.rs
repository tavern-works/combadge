use std::any::type_name;

use js_sys::Array;
use wasm_bindgen::JsValue;

use crate::{Error, Post};

pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> From<core::result::Result<T, E>> for Result<T, E> {
    fn from(value: core::result::Result<T, E>) -> Self {
        match value {
            Ok(value) => Self::Ok(value),
            Err(error) => Self::Err(error),
        }
    }
}

impl<T, E> Into<core::result::Result<T, E>> for Result<T, E> {
    fn into(self) -> core::result::Result<T, E> {
        match self {
            Self::Ok(value) => Ok(value),
            Self::Err(error) => Err(error),
        }
    }
}

impl<T, E> Post for Result<T, E> {
    fn from_js_value(value: JsValue) -> core::result::Result<Self, Error> {
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

    fn to_js_value(self) -> core::result::Result<JsValue, Error> {
        match self {
            Self::Ok(value) => {
                Ok(Array::of2(&JsValue::from_str("Ok"), &value.to_js_value()?).into())
            }
            Self::Err(error) => {
                Ok(Array::of2(&JsValue::from_str("Err"), &error.to_js_value()?).into())
            }
        }
    }
}
