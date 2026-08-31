use crate::{events::event_emit_bal_update, call_accounts, Imm, OfframpArgs, Sig, storage, call_eip20, signable::SignableOfframpArgs};

use bobcat_sdk::{entry::msg_sender, maths::U};

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
    assert_eq!(msg_sender(), acc_addr, "not owner of blob submitted");
    let acc_addr_u: U = U::from(acc_addr);
    storage::server_ms_ts::exchange_res(&acc_addr_u, &ms_ts.into(), "server ms ts").unwrap();
    if *should_server_sig_validate {
        let SignableOfframpArgs { hash } = SignableOfframpArgs::from(&args);
        assert!(
            bobcat_sdk::precompiles::superposition::edphverify_post(hash, *server_key, server_sig),
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
    let recipient = call_accounts::get_owner(acc_addr).unwrap();
    assert!(call_eip20::transfer(asset.into(), recipient, &amt_u), "transfer failed");
    0
}
