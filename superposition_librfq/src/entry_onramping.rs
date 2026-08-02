use crate::{call_eip20, immutables::Imm};

use bobcat_sdk::{
    entry::{contract_address, msg_sender},
    maths::U,
};

pub fn entry_onramping(&Imm { addr_fusdc, .. }: &Imm, owner_passport: [u8; 20], amt: u64) -> usize {
    // Handle the onramping of a user's balance, increasing the amount that
    // can be spent by a user before they interact with the system. The
    // amount that we increase it by is the amount that's increased after a
    // deposit to AAVE V3 for USDC.
    let amt_u = U::from(amt);
    assert!(
        call_eip20::transfer_from(addr_fusdc, msg_sender(), contract_address(), &amt_u),
        "transfer from failed"
    );
    0
}
