use core::fmt::{Formatter, Result as FmtResult, Debug, Display};

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
    SenderNotMaker,
    HashAlreadySeen,
}

#[derive(Clone, PartialEq)]
pub struct Error {
    pub dis: Dis,
    pub hash: Option<U>,
    pub storage: Option<&'static str>,
    pub consuming_amt: Option<U>,
    pub prev_amt: Option<U>,
    pub hint: Option<&'static str>,
}

impl From<Dis> for Error {
    fn from(dis: Dis) -> Error {
        Error {
            dis,
            hash: None,
            storage: None,
            consuming_amt: None,
            prev_amt: None,
            hint: None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{self:?}")
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> FmtResult {
        let Self { dis, hash, storage, consuming_amt, prev_amt, hint } = self;
        write!(f, "Error {{ dis: {dis:?}, hash: {hash:?}, storage: {storage:?}, consuming_amt: ")?;
        if let Some(x) = consuming_amt {
            write!(f, "{x}")?;
        } else {
            write!(f, "None")?;
        };
        write!(f, ", prev_amt: ")?;
        if let Some(x) = prev_amt {
            write!(f, "{x}")?;
        } else {
            write!(f, "None")?;
        };
        write!(f, ", hint: {hint:?} }}")
    }
}

impl core::error::Error for Error {}
