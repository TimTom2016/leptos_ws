use thiserror::Error;

#[derive(Error,Debug)]
pub enum Error {
    #[error("No ServerSignals in State")]
    MissingServerSignals,
    #[error("Could not add ServerSignal to ServerSignals")]
    AddingSignalFailed,
    #[error(transparent)]
    SerializationFailed(#[from] serde_json::Error),

}