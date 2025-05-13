use anyhow::anyhow;
use combadge::Post;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Error {
    internal: anyhow::Error,
}

impl Error {
    #[must_use]
    pub fn into_internal(self) -> anyhow::Error {
        self.internal
    }
}

#[wasm_bindgen]
impl Error {
    #[wasm_bindgen(js_name = toString)]
    #[must_use]
    pub fn js_to_string(&self) -> String {
        format!("{:#}", self.internal)
    }
}

impl Post for Error {
    const POSTABLE: bool = true;

    fn from_js_value(value: JsValue) -> Result<Self, combadge::Error> {
        let string = value
            .as_string()
            .ok_or_else(|| combadge::Error::DeserializeFailed {
                type_name: String::from("Error"),
                error: String::from("failed to extract string for error"),
            })?;
        Ok(Self::from(anyhow!(string)))
    }

    fn to_js_value(self) -> Result<JsValue, combadge::Error> {
        Ok(self.into_internal().to_string().into())
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Self { internal: error }
    }
}

impl From<combadge::Error> for Error {
    fn from(error: combadge::Error) -> Self {
        Self {
            internal: anyhow!(error),
        }
    }
}
