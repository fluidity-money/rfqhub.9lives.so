use crate::{TOPIC_BALANCE_CHANGED, TOPIC_GLOBAL_BALANCE_CHANGED, call_eip20, storage};

use bobcat_sdk::{
    entry::{contract_address, msg_sender},
    events::emit,
    maths::U,
};

use superposition_assets::Asset;

pub fn entry_onramping_from_bal(asset: Asset, amt: u64) -> usize {
    // Handle the onramping of a user's balance, increasing the amount that
    // can be spent by a user before they interact with the system.
    let amt_u = U::from(amt);
    let sender = msg_sender();
    let sender_u = U::from(sender);
    let our_addr = contract_address();
    let asset_addr: [u8; 20] = asset.into();
    let asset_addr_u: U = asset_addr.into();
    assert!(
        call_eip20::safe_transfer_from(asset_addr, sender, our_addr, &amt_u),
        "transfer from failed"
    );
    let user_bal = storage::onramped_bal::checked_add_res(
        &asset_addr_u,
        &sender_u,
        &amt_u,
        "onramping from bal",
    )
    .unwrap();
    let global_bal =
        storage::global_bal::checked_add_res(&asset_addr_u, &amt_u, "incresaing global bal")
            .unwrap();
    emit!(TOPIC_BALANCE_CHANGED, sender, asset_addr, user_bal);
    emit!(TOPIC_GLOBAL_BALANCE_CHANGED, asset_addr, global_bal);
    0
}
