use bobcat_sdk::{
    call::{safe_call_unit, safe_static_call_slice},
    maths::U,
};

use superposition_libaccounts::{Args, Sig, StatementArgs};

use crate::error::{Dis, Error, ErrorData};

use alloc::boxed::Box;

pub fn get_owner(addr: [u8; 20]) -> Result<[u8; 20], Error> {
    let cd = borsh::to_vec(&Args::Owner).unwrap();
    let (ok, _, a) = safe_static_call_slice::<20>(addr, &cd, u64::MAX, 0)
        .ok_or(Error::from(Dis::AccountNonexistent))?;
    if !ok {
        return Err(Error {
            dis: Dis::AccountError,
            data: Box::new(ErrorData {
                acc_addr: Some(addr),
                cd: Some(cd),
                ..ErrorData::default()
            }),
        });
    }
    Ok(a)
}

/// Check the statement of a hash given by a user by converting it to
/// its keccak form then checking to see if they signed it with the
/// prelude that we use internally.
pub fn check_hash_statement(addr: [u8; 20], hash: U, ms_ts: [u8; 6], sig: &[u8; 64]) -> bool {
    safe_call_unit(
        addr,
        &borsh::to_vec(&Args::Statement {
            args: StatementArgs {
                msg: hash.as_vec(),
                ms_ts,
            },
            sig: Sig(*sig),
        })
        .unwrap(),
        &U::ZERO,
        u64::MAX,
    )
}
