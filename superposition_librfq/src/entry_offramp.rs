// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use crate::{
    Imm, OfframpArgs, Sig, call_eip20, events::event_emit_bal_update,
    signable::SignableOfframpArgs, storage,
};

#[cfg(not(feature = "arbos"))]
use crate::call_accounts;

use bobcat_sdk::{
    entry::msg_sender,
    maths::U,
    storage::{const_keccak256, slot_map, storage_load},
};

pub fn entry_offramp(
    Imm {
        should_server_sig_validate,
        server_key,
        ..
    }: &Imm,
    args: OfframpArgs,
    server_sig: Sig,
) -> usize {
    let OfframpArgs {
        asset,
        amt,
        ms_ts,
        acc_addr,
    } = args;
    assert!(amt > 0, "offramp balance is zero");
    let acc_owner = get_acc_owner(acc_addr);
    assert_eq!(msg_sender(), acc_owner, "not owner of account for offramp");
    let acc_addr_u: U = U::from(acc_addr);
    storage::server_ms_ts::exchange_res(&acc_addr_u, &ms_ts.into(), "server ms ts").unwrap();
    if *should_server_sig_validate {
        let SignableOfframpArgs { hash } = SignableOfframpArgs::from(&args);
        assert!(
            bobcat_sdk::precompiles::superposition::edphverify_post(
                hash,
                *server_key,
                server_sig.0
            ),
            "bad signature"
        );
    }
    let asset_u: U = asset.u();
    let existing_bal = storage::onramped_bal::get(&asset_u, &acc_addr_u);
    let amt_u = amt.into();
    assert!(existing_bal >= amt_u, "not enough balance for offramp");
    let remaining_bal = existing_bal - amt_u;
    storage::onramped_bal::set(&asset_u, &acc_addr_u, &remaining_bal);
    let global_bal = storage::global_bal::checked_sub_res(&asset_u, &amt_u, "global bal").unwrap();
    event_emit_bal_update(acc_addr, asset.into(), remaining_bal, global_bal);
    assert!(
        call_eip20::transfer(asset.into(), acc_owner, &amt_u),
        "transfer failed"
    );
    0
}

// Get the owner of an account, or skipping it based on what's in the
// current slot for this purpose.
fn get_acc_owner(acc: [u8; 20]) -> [u8; 20] {
    #[cfg(feature = "arbos")]
    unsafe {
        arbos_recipient_hint(acc)
    }
    #[cfg(not(feature = "arbos"))]
    call_accounts::get_owner(acc).unwrap()
}

// Not safe for production, checks a derived storage slot that the test
// runner can set for online testing.
#[allow(unused)]
unsafe fn arbos_recipient_hint(acc: [u8; 20]) -> [u8; 20] {
    #[cfg(all(feature = "arbos", target_arch = "wasm32"))]
    assert_eq!(
        31337,
        bobcat_sdk::entry::chain_id(),
        "chain id not arbos-forge"
    );
    storage_load(&slot_map(&acc.into(), &const_keccak256(b"arbos.recipient"))).into()
}
