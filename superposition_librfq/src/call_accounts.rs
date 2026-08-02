use bobcat_sdk::{call::call_unit, maths::U};

use superposition_libaccounts::{Args, Sig, StatementArgs};

use alloc::string::String;

/// Check the statement of a signer using the accounts address given.
/// Verify that the address has code before making this call.
pub fn check_statement(addr: [u8; 20], slot: u32, msg: String, ms_ts: [u8; 16], sig: Sig) -> bool {
    call_unit(
        addr,
        &borsh::to_vec(&Args::Statement {
            slot,
            args: StatementArgs { msg, ms_ts },
            sig,
        })
        .unwrap(),
        &U::ZERO,
        u64::MAX,
    )
}
