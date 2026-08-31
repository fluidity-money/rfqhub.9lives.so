use bobcat_sdk::{
    call::{safe_static_call_slice, safe_static_call_unit},
    maths::U,
};

use superposition_libaccounts::{Args, Sig, StatementArgs};

use crate::{error::{Error, Dis}, signable::Hash};

pub fn get_owner(addr: [u8; 20]) -> Result<[u8; 20], Error> {
    let (ok, _, addr) = safe_static_call_slice::<20>(
        addr,
        &borsh::to_vec(&Args::Owner).unwrap(),
        u64::MAX,
        0,
    ).ok_or(Error::from(Dis::AccountNonexistent))?;
    if !ok {
        return Err(Dis::AccountError.into());
    }
    Ok(addr)
}

/// Check the statement of a hash given by a user by converting it to
/// its keccak form then checking to see if they signed it with the
/// prelude that we use internally.
pub fn check_hash_statement(addr: [u8; 20], hash: Hash, ms_ts: [u8; 6], sig: &[u8; 64]) -> bool {
    let hash: U = hash.into();
    safe_static_call_unit(
        addr,
        &borsh::to_vec(&Args::Statement {
            args: StatementArgs {
                msg: hash.0.to_vec(),
                ms_ts,
            },
            sig: Sig(*sig),
        })
        .unwrap(),
        u64::MAX,
    )
}
