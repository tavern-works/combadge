use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("client unavailable")]
    ClientUnavailable,

    #[error("failed to deserialize type {type_name}: {error}")]
    FailedToDeserialize { type_name: String, error: String },

    #[error("failed to serialize type {type_name}: {error}")]
    FailedToSerialize { type_name: String, error: String },
}
