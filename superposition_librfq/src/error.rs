use alloc::boxed::Box;
use core::fmt::{Debug, Display, Formatter, Result as FmtResult};

use bobcat_sdk::maths::U;

#[derive(Debug, Clone, PartialEq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
    pub data: Box<ErrorData>,
}

#[derive(Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ErrorData {
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
            data: Box::default(),
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
        let Self { dis, data } = self;
        let ErrorData {
            hash,
            storage,
            consuming_amt,
            prev_amt,
            hint,
        } = data.as_ref();
        write!(
            f,
            "Error {{ dis: {dis:?}, hash: {hash:?}, storage: {storage:?}, consuming_amt: "
        )?;
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
