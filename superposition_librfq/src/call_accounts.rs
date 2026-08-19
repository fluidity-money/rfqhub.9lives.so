use bobcat_sdk::{
    call::{call_slice, call_unit},
    maths::U,
};

use superposition_libaccounts::{Args, Sig, StatementArgs};

use crate::signable::Hash;

pub fn get_owner(addr: [u8; 20]) -> [u8; 20] {
    let (rc, _, o) = call_slice::<20>(
        addr,
        &borsh::to_vec(&Args::Owner).unwrap(),
        &U::ZERO,
        u64::MAX,
        0,
    );
    assert!(rc, "reverted owner call");
    o
}

/// Check the statement of a hash given by a user by converting it to
/// its keccak form then checking to see if they signed it with the
/// prelude that we use internally.
pub fn check_hash_statement(addr: [u8; 20], hash: Hash, ms_ts: [u8; 6], sig: &[u8; 64]) -> bool {
    let hash: U = hash.into();
    call_unit(
        addr,
        &borsh::to_vec(&Args::Statement {
            args: StatementArgs {
                msg: hash.0.to_vec(),
                ms_ts,
            },
            sig: Sig(*sig),
        })
        .unwrap(),
        &U::ZERO,
        u64::MAX,
    )
}
