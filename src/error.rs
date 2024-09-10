use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("callback failed")]
    CallbackFailed { error: String },

    #[error("client unavailable")]
    ClientUnavailable,

    #[error("failed to create {type_name}: {error}")]
    CreationFailed { type_name: String, error: String },

    #[error("failed to deserialize type {type_name}: {error}")]
    DeserializeFailed { type_name: String, error: String },

    #[error("failed to post message: {error}")]
    PostFailed { error: String },

    #[error("failed to receive message: {error}")]
    ReceiveFailed { error: String },

    #[error("failed to serialize type {type_name}: {error}")]
    SerializeFailed { type_name: String, error: String },

    #[error("unknown procedure {name}")]
    UnknownProcedure { name: String },

    #[error("unsupported type {name} (types need to either be Into<JsValue> and From<JsValue> or [de]serializable with serde)")]
    UnsupportedType { name: String },
}
