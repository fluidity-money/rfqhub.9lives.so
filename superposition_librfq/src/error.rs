// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use alloc::{boxed::Box, vec::Vec};

use crate::Sig;

use core::fmt::{Debug, Display, Formatter, Result as FmtResult};

use bobcat_sdk::maths::U;

#[derive(Debug, Clone, PartialEq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Dis {
    TimestampletInconsistentOutcome,
    TimestampletInconsistentTime,
    BadAsset,
    InconsistentSignable,
    StatementCheckFailed,
    BadSignature,
    CheckedAddOverflow,
    CheckedSubUnderflow,
    SenderNotTaker,
    SenderNotMaker,
    HashAlreadySeen,
    ExchangeErr,
    AccountNonexistent,
    AccountError,
    InconsistentAsset,
    InconsistentAccount,
    EmptyOnrampingSrc,
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
    pub old_val: Option<U>,
    pub server_key: Option<U>,
    pub acc_addr: Option<[u8; 20]>,
    pub acc_ms_ts: Option<[u8; 6]>,
    pub acc_sig: Option<Sig>,
    pub acc_msg: Option<U>,
    pub cd: Option<Vec<u8>>,
    pub sender: Option<[u8; 20]>,
    pub expected_sender: Option<[u8; 20]>,
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
            old_val,
            server_key,
            acc_addr,
            acc_ms_ts,
            acc_sig,
            acc_msg,
            cd,
            sender,
            expected_sender,
        } = data.as_ref();
        write!(
            f,
            "Error {{ dis: {dis:?}, hash: {hash:?}, storage: {storage:?}, consuming_amt: "
        )?;
        if let Some(x) = consuming_amt {
            write!(f, "{x}")
        } else {
            write!(f, "None")
        }?;
        write!(f, ", prev_amt: ")?;
        if let Some(x) = prev_amt {
            write!(f, "{x}")
        } else {
            write!(f, "None")
        }?;
        write!(f, ", hint: {hint:?}, old_val: ")?;
        if let Some(x) = old_val {
            write!(f, "{x}")
        } else {
            write!(f, "None")
        }?;
        write!(f, ", server_key: ")?;
        if let Some(x) = server_key {
            write!(f, "{x}")
        } else {
            write!(f, "None")
        }?;
        write!(f, ", acc_addr: ")?;
        if let Some(x) = acc_addr {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, ", acc_ms_ts: ")?;
        if let Some(x) = acc_ms_ts {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, ", acc_sig: ")?;
        if let Some(Sig(x)) = acc_sig {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, ", acc_msg: ")?;
        if let Some(x) = acc_msg {
            write!(f, "{x:?}")
        } else {
            write!(f, "None")
        }?;
        write!(f, ", cd: ")?;
        if let Some(x) = cd {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, ", sender: ")?;
        if let Some(x) = sender {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, ", expected_sender: ")?;
        if let Some(x) = expected_sender {
            write!(f, "{}", const_hex::encode(x))
        } else {
            write!(f, "None")
        }?;
        write!(f, "}}")
    }
}

impl core::error::Error for Error {}
