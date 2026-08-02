use crate::{storage, call_eip20, immutables::Imm, call_aave_v3};

use bobcat_sdk::{
    entry::{contract_address, msg_sender},
    maths::U,
};

pub fn entry_onramping(
    &Imm {
        addr_fusdc,
        addr_aave_v3,
        ..
    }: &Imm,
    owner_passport: [u8; 20],
    amt: u64,
) -> usize {
    // Handle the onramping of a user's balance, increasing the amount that
    // can be spent by a user before they interact with the system. The
    // amount that we increase it by is the amount that's increased after a
    // deposit to AAVE V3 for USDC.
    let amt_u = U::from(amt);
    let sender = msg_sender();
    let our_addr = contract_address();
    assert!(
        call_eip20::transfer_from(addr_fusdc, sender, our_addr, &amt_u),
        "transfer from failed"
    );
    assert!(
        call_aave_v3::supply(addr_aave_v3, &amt_u, our_addr, 0),
        "aave supply failed"
    );
    // The cTokens from this function would be the same as what we supplied:
    storage::onramped_bal::checked_add(&U::from(sender), &amt_u);
    storage::global_bal::checked_add(&amt_u);
    // Now that we've taken their deposit, we just need to count this for
    // when someone requests it from the entrypoint. We don't emit any events
    // here for the sake of keeping it simple.
    0
}
