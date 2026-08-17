use crate::storage::BadAsset;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Dis {
    TimestampletInconsistentOutcome,
    TimestampletInconsistentTime,
    BadAssetNumber,
    InconsistentSignable,
    StatementCheckFailed,
    BadSignature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub dis: Dis,
}

impl From<Dis> for Error {
    fn from(dis: Dis) -> Error {
        Error { dis }
    }
}

impl From<BadAsset> for Dis {
    fn from(_: BadAsset) -> Self {
        Dis::BadAssetNumber
    }
}

impl From<BadAsset> for Error {
    fn from(x: BadAsset) -> Self {
        Error { dis: x.into() }
    }
}
