use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SphinxError {
    #[error("packet is not the expected size or shape")]
    MalformedPacket,
    #[error("header integrity check failed")]
    IntegrityFailure,
    #[error("payload authentication failed")]
    Aead,
    #[error("path of {0} hops is outside 1..={max}", max = crate::MAX_HOPS)]
    PathLength(usize),
    #[error("message of {len} bytes exceeds the {max} byte payload")]
    MessageTooLong { len: usize, max: usize },
}
