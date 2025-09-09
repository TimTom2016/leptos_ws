use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("No ServerSignals in State")]
    MissingServerSignals,
    #[error("Could not add ServerSignal to ServerSignals")]
    AddingSignalFailed,
    #[error("Could not add ChannelHandler to ChannelHandlers")]
    AddingChannelHandlerFailed,

    #[error("Could not update Signal")]
    UpdateSignalFailed,

    #[error("Method not available on Signal")]
    NotAvailableOnSignal,

    #[error(transparent)]
    SerializationFailed(#[from] serde_json::Error),
}
