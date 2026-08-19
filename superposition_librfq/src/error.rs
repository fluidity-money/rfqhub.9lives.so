#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Dis {
    TimestampletInconsistentOutcome,
    TimestampletInconsistentTime,
    BadAssetNumber,
    InconsistentSignable,
    StatementCheckFailed,
    BadSignature,
    CheckedAddOverflow,
    CheckedSubUnderflow,
    SenderNotTaker,
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
