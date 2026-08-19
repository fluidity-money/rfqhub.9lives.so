use bobcat_sdk::maths::U;

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
    HashAlreadySeen,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub dis: Dis,
    pub hash: Option<U>,
    pub storage: Option<&'static str>,
    pub prev_amt: Option<U>,
    pub hint: Option<&'static str>,
}

impl From<Dis> for Error {
    fn from(dis: Dis) -> Error {
        Error {
            dis,
            hash: None,
            storage: None,
            prev_amt: None,
            hint: None,
        }
    }
}
