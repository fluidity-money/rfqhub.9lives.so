use crate::{
    AggregateArgs, AggregateEnum, AggregatedResultArgs, BundleMaker, BundleMakerArgs, BundleTaker,
    BundleTakerArgs, Imm, OnchainPotArgs, OnrampingSource, Sig, Timestamplet, call_eip20,
    error::{Dis, Error, ErrorData},
    events::event_emit_bal_update,
    signable::{Hash, SignableAggregateArgs},
    storage,
};

#[cfg(not(feature = "arbos"))]
use crate::call_accounts;

use alloc::boxed::Box;

use bobcat_sdk::{
    entry::{chain_id, msg_sender},
    maths::U,
    storage::{const_keccak256, slot_map, storage_load},
};

// Recursively descend into the aggregated bundle, returning the address
// of the taker, and the amount they provided.
pub unsafe fn recurse_aggregate_src(
    imm: &Imm,
    src: &OnrampingSource,
    aggregated_res: Option<&SignableAggregateArgs>,
) -> Result<(u64, [u8; 20], [u8; 20]), Error> {
    match src {
        OnrampingSource::OnchainPot(OnchainPotArgs {
            acc_addr,
            amt,
            asset,
        }) => {
            let asset_u: U = asset.addr().into();
            let amt_u = U::from(*amt);
            let user_bal = storage::onramped_bal::checked_sub_res(
                &asset_u,
                &acc_addr.into(),
                &amt_u,
                "onchain pot source",
            )?;
            event_emit_bal_update(
                *acc_addr,
                asset.addr(),
                user_bal,
                storage::global_bal::get(&asset_u),
            );
            Ok((*amt, *acc_addr, asset.addr()))
        }
        OnrampingSource::AggregatedResult(AggregatedResultArgs { from, amt_to_take }) => {
            let (available, acc, asset, source_hash) = match from.as_ref() {
                AggregateEnum::Args(agg) => unsafe {
                    let res = aggregated_res.ok_or(Dis::InconsistentSignable)?;
                    let (available, acc, asset) =
                        recurse_aggregate_args(imm, &agg.args, res, &agg.server_sig)?;
                    (available, acc, asset, res.hash.clone().into())
                },
                AggregateEnum::Hash(h) => {
                    let asset = storage::hash_asset::get(h);
                    (
                        storage::hash_taker_leftover::get(h, &asset).into(),
                        storage::hash_taker_addr::get(h).into(),
                        asset.into(),
                        *h,
                    )
                }
            };
            let amount_to_onramp = available
                .checked_sub(*amt_to_take)
                .ok_or(Dis::CheckedSubUnderflow)?;
            let asset_u = U::from(asset);
            storage::hash_taker_leftover::set(&source_hash, &asset_u, &U::ZERO);
            let user_bal = storage::onramped_bal::checked_add_res(
                &asset_u,
                &acc.into(),
                &amount_to_onramp.into(),
                "aggregated result remainder to onramped bal",
            )?;
            event_emit_bal_update(acc, asset, user_bal, storage::global_bal::get(&asset_u));
            Ok((*amt_to_take, acc, asset))
        }
    }
}

// Recursively descend into the aggregated bundle, updating the storage
// amounts as we go. The caller should check the storage of the users
// they're draining the liquidity from. This function will zero out the
// amounts left over in the previous slot hash as it goes, if any. There is
// an internal concept of the hashes stored at different aggregate points
// to tolerate market makers taking their funds out as they're participating
// in different markets.
// SAFETY: Uses the contract storage to move amounts around, taking
// advantage of the memory performing caching at places. Uses the storage
// to look up the funds involved.
pub unsafe fn recurse_aggregate_args(
    imm: &Imm,
    &AggregateArgs {
        maker:
            BundleMaker {
                args:
                    BundleMakerArgs {
                        src: ref maker_src,
                        taker:
                            BundleTaker {
                                args:
                                    BundleTakerArgs {
                                        ref src,
                                        expiry,
                                        ms_ts: taker_ms_ts,
                                        is_up,
                                        price_target,
                                        outcome,
                                    },
                                sig: taker_sig,
                            },
                        ms_ts: maker_ms_ts,
                        acc: maker_acc,
                    },
                sig: maker_sig,
            },
        timestamplet:
            Timestamplet {
                timestamplet_time,
                timestamplet_outcome,
                timestamplet_val,
            },
    }: &AggregateArgs,
    signable: &SignableAggregateArgs,
    server_sig: &Sig,
) -> Result<(u64, [u8; 20], [u8; 20]), Error> {
    if timestamplet_outcome != outcome {
        return Err(Dis::TimestampletInconsistentOutcome.into());
    }
    if timestamplet_time != expiry {
        return Err(Dis::TimestampletInconsistentTime.into());
    }
    if imm.should_server_sig_validate {
        check_server_sig(&signable.hash, &imm.server_key, *server_sig)?;
        // Here, we don't bother checking the server signature since the
        // timestamps contained within are going to protect us here
        // from abuse.
    }
    let signable_hash_u = U::from(signable.hash.clone());
    // Check if we've actually seen this hash before, and if we have, bug
    // out. They need to submit with the hash again themselves:
    if storage::hash_asset::get(&signable_hash_u).is_some() {
        return Err(Error {
            dis: Dis::HashAlreadySeen,
            data: Box::new(ErrorData {
                hash: Some(signable_hash_u),
                ..Default::default()
            }),
        });
    }
    let (taker_amt, taker_acc, asset) = unsafe {
        recurse_aggregate_src(
            imm,
            src,
            signable.maker.taker_args.src.aggregated_res.as_deref(),
        )?
    };
    let (maker_amt, maker_src_acc, maker_asset) = unsafe {
        recurse_aggregate_src(imm, maker_src, signable.maker.src.aggregated_res.as_deref())?
    };
    if maker_src_acc != maker_acc {
        return Err(Dis::InconsistentAccount.into());
    }
    if maker_asset != asset {
        return Err(Dis::InconsistentAsset.into());
    }
    let taker_acc_u: U = taker_acc.into();
    let maker_acc_u: U = maker_acc.into();
    storage::ms_ts::exchange_res(&taker_acc_u, &taker_ms_ts.into(), "taker ms ts")?;
    storage::ms_ts::exchange_res(&maker_acc_u, &maker_ms_ts.into(), "maker ms ts")?;
    if imm.should_user_sig_validate {
        check_hash_signer(
            maker_acc,
            signable.maker.hash.clone(),
            maker_ms_ts,
            maker_sig,
        )?;
        check_hash_signer(
            taker_acc,
            signable.maker.taker_args.hash.clone(),
            taker_ms_ts,
            taker_sig,
        )?;
    }
    let did_taker_win = match (is_up, timestamplet_val > price_target) {
        (true, true) | (false, false) => true,
        _ => false,
    };
    let asset_u = U::from(asset);
    let maker_acc_u = U::from(maker_acc);
    storage::hash_taker_addr::set(&signable_hash_u, &taker_acc_u);
    storage::hash_maker_addr::set(&signable_hash_u, &maker_acc_u);
    let taker_leftover = if did_taker_win {
        taker_amt
            .checked_add(maker_amt)
            .ok_or(Dis::CheckedAddOverflow)?
    } else {
        0
    };
    if did_taker_win {
        storage::hash_taker_leftover::set(&signable_hash_u, &asset_u, &U::from(taker_leftover));
    } else {
        let maker_winnings = taker_amt
            .checked_add(maker_amt)
            .ok_or(Dis::CheckedAddOverflow)?;
        storage::hash_maker_leftover::checked_add_res(
            &signable_hash_u,
            &asset_u,
            &U::from(maker_winnings),
            "hash maker leftover checked add",
        )?;
    }
    storage::hash_asset::set(&signable_hash_u, &asset_u);
    Ok((taker_leftover, taker_acc, asset))
}

// Entry the aggregate recursive function, returning the amount that's
// able to be withdrawn by the party given. Checks the msg_sender.
pub fn entry_recurse_aggregate_enum(
    imm: &Imm,
    args: &AggregateEnum,
    is_taker: bool,
) -> Result<([u8; 20], [u8; 20], u64), Error> {
    let sender = msg_sender();
    match args {
        AggregateEnum::Args(agg) => {
            let signable = SignableAggregateArgs::from(&agg.args);
            let (taker_leftover, taker_acc, asset) =
                unsafe { recurse_aggregate_args(imm, &agg.args, &signable, &agg.server_sig)? };
            if is_taker {
                let recipient = get_acc_owner(taker_acc)?;
                if recipient != sender {
                    return Err(Dis::SenderNotTaker.into());
                }
                storage::hash_taker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok((sender, asset, taker_leftover))
            } else {
                let signable_hash_u: U = signable.hash.clone().into();
                let maker_acc: [u8; 20] = storage::hash_maker_addr::get(&signable_hash_u).into();
                let recipient = get_acc_owner(maker_acc)?;
                if recipient != sender {
                    return Err(Dis::SenderNotTaker.into());
                }
                let maker_leftover =
                    storage::hash_maker_leftover::get(&signable_hash_u, &asset.into());
                storage::hash_maker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok((sender, asset, maker_leftover.into()))
            }
        }
        AggregateEnum::Hash(h) => {
            let asset = storage::hash_asset::get(h);
            let (sender, asset, amt) = if is_taker {
                let amt = storage::hash_taker_leftover::get(h, &asset);
                let recipient = get_acc_owner(storage::hash_taker_addr::get(h).into())?;
                if recipient != sender {
                    return Err(Dis::SenderNotTaker.into());
                }
                storage::hash_taker_leftover::set(h, &asset, &U::ZERO);
                (sender, asset, amt)
            } else {
                let recipient = get_acc_owner(storage::hash_maker_addr::get(h).into())?;
                if recipient != sender {
                    return Err(Dis::SenderNotMaker.into());
                }
                let amt = storage::hash_maker_leftover::get(h, &asset);
                storage::hash_maker_leftover::set(h, &asset, &U::ZERO);
                (sender, asset, amt)
            };
            Ok((sender, asset.into(), amt.into()))
        }
    }
}

pub fn entry_aggregate_to_offramp(imm: &Imm, args: AggregateEnum, is_taker: bool) -> usize {
    // This code recursively descends inside the aggregate args, finding the
    // correct winning amount for the maker and takers, crediting the winning
    // addresses correctly. The amounts are then sent to the users.
    let (sender, asset, leftover) = entry_recurse_aggregate_enum(imm, &args, is_taker).unwrap();
    let hash = match &args {
        AggregateEnum::Args(agg) => SignableAggregateArgs::from(&agg.args).hash.into(),
        AggregateEnum::Hash(hash) => *hash,
    };
    let acc_addr: [u8; 20] = if is_taker {
        storage::hash_taker_addr::get(&hash).into()
    } else {
        storage::hash_maker_addr::get(&hash).into()
    };
    let asset_u = U::from(asset);
    let leftover_u = U::from(leftover);
    let global_bal =
        storage::global_bal::checked_sub_res(&asset_u, &leftover_u, "aggregate offramp").unwrap();
    event_emit_bal_update(
        acc_addr,
        asset,
        storage::onramped_bal::get(&asset_u, &acc_addr.into()),
        global_bal,
    );
    assert!(
        call_eip20::transfer(asset, sender, &leftover_u),
        "eip20 transfer err"
    );
    0
}

fn check_hash_signer(
    _addr: [u8; 20],
    _hash: Hash,
    _ms_ts: [u8; 6],
    _sig: Sig,
) -> Result<(), Error> {
    #[cfg(feature = "arbos")]
    {
        unsafe { check_on_arbos() };
        Ok(())
    }
    #[cfg(not(feature = "arbos"))]
    if call_accounts::check_hash_statement(_addr, _hash, _ms_ts, &_sig) {
        Ok(())
    } else {
        Err(Dis::StatementCheckFailed.into())
    }
}

fn check_server_sig(hash: &Hash, key: &U, sig: [u8; 64]) -> Result<(), Error> {
    if let Hash::JustInTime(hash) = hash {
        let is_sig_ok = bobcat_sdk::precompiles::superposition::edphverify_post(*hash, *key, sig);
        if is_sig_ok {
            Ok(())
        } else {
            Err(Dis::BadSignature.into())
        }
    } else {
        // The hash is already on-chain, we don't need to verify it.
        Ok(())
    }
}

// Get the owner of an account, or skipping it based on what's in the
// current slot for this purpose.
fn get_acc_owner(acc: [u8; 20]) -> Result<[u8; 20], Error> {
    #[cfg(feature = "arbos")]
    unsafe {
        Ok(arbos_recipient_hint(acc))
    }
    #[cfg(not(feature = "arbos"))]
    call_accounts::get_owner(acc)
}

unsafe fn check_on_arbos() {
    assert_eq!(31337, chain_id(), "chain id not arbos-forge");
}

// Not safe for production, checks a derived storage slot that the test
// runner can set for online testing.
#[allow(unused)]
unsafe fn arbos_recipient_hint(acc: [u8; 20]) -> [u8; 20] {
    unsafe { check_on_arbos() };
    storage_load(&slot_map(&acc.into(), &const_keccak256(b"arbos.recipient"))).into()
}
