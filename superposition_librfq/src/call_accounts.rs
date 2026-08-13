use bobcat_sdk::{call::call_unit, maths::U};

use superposition_libaccounts::{Args, Sig, StatementArgs};

use alloc::vec::Vec;

/// Check the statement of a signer using the accounts address given.
/// Verify that the address has code before making this call.
pub fn check_statement(addr: [u8; 20], msg: Vec<u8>, ms_ts: [u8; 6], sig: Sig) -> bool {
    call_unit(
        addr,
        &borsh::to_vec(&Args::Statement {
            args: StatementArgs { msg, ms_ts },
            sig,
        })
        .unwrap(),
        &U::ZERO,
        u64::MAX,
    )
}
