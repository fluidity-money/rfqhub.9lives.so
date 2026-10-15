use crate::{
    AggregateArgs, AggregateEnum, AggregateToOfframpArg, AggregatedResultArgs, BundleMaker,
    BundleMakerArgs, BundleTaker, BundleTakerArgs, Imm, OnchainPotArgs, OnrampingSource, Sig,
    Timestamplet, call_eip20,
    error::{Dis, Error, ErrorData},
    events::event_emit_bal_update,
    signable::{Hash, SignableAggregateArgs, SignableOnrampingSource},
    storage,
};

#[cfg(not(feature = "arbos"))]
use crate::call_accounts;

use alloc::boxed::Box;

use bobcat_sdk::{
    bobcat_trace,
    entry::{chain_id, msg_sender},
    maths::U,
    storage::{const_keccak256, slot_map, storage_load},
};

use superposition_assets::Asset;

fn recurse_aggregate_acc_asset(src: &[OnrampingSource]) -> Result<([u8; 20], Asset), Error> {
    match src.iter().next().ok_or(Dis::EmptyOnrampingSrc)? {
        OnrampingSource::OnchainPot(OnchainPotArgs {
            acc_addr, asset, ..
        }) => Ok((*acc_addr, *asset)),
        OnrampingSource::AggregatedResult(AggregatedResultArgs { from, .. }) => {
            match from.as_ref() {
                AggregateEnum::Args(agg) => {
                    let maker = &agg.args.maker.args;
                    let taker = &maker.taker.args;
                    // The source winner is independent of the new consumer's role.
                    let did_taker_win = taker.is_up
                        == (agg.args.timestamplet.timestamplet_val > taker.price_target);
                    recurse_aggregate_acc_asset(if did_taker_win {
                        &taker.src
                    } else {
                        &maker.src
                    })
                }
                AggregateEnum::Hash(h) => {
                    let asset_word = storage::hash_asset::get(h);
                    let asset = [Asset::USDC, Asset::ARB, Asset::WETH]
                        .into_iter()
                        .find(|asset| U::from(asset.addr()) == asset_word)
                        .ok_or(Dis::BadAsset)?;
                    let (_, acc, _) = aggregate_hash_winner(h, &asset_word)?;
                    Ok((acc, asset))
                }
            }
        }
    }
}

// A settled aggregate has exactly one funded side. Reject consumed/unknown
// hashes (including zero-amount replays), and never guess for ambiguous storage.
fn aggregate_hash_winner(h: &U, asset: &U) -> Result<(u64, [u8; 20], bool), Error> {
    let taker = storage::hash_taker_leftover::get(h, asset);
    let maker = storage::hash_maker_leftover::get(h, asset);
    match (taker.is_some(), maker.is_some()) {
        (true, false) => Ok((taker.into(), storage::hash_taker_addr::get(h).into(), true)),
        (false, true) => Ok((maker.into(), storage::hash_maker_addr::get(h).into(), false)),
        _ => Err(Dis::CheckedSubUnderflow.into()),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecurseAggregateSrc {
    pub taker_amt: u64,
    pub maker_amt: u64,
    pub taker_acc: [u8; 20],
    pub asset: [u8; 20],
    pub maker_acc: [u8; 20],
}

// Recursively descend into the aggregated bundle.
#[bobcat_trace]
pub unsafe fn recurse_aggregate_src(
    imm: &Imm,
    src: &[OnrampingSource],
    aggregated_onramping_src: &[SignableOnrampingSource],
    is_taker: bool,
) -> Result<RecurseAggregateSrc, Error> {
    let (exp_acc, exp_asset) = recurse_aggregate_acc_asset(src)?;
    if src.len() != aggregated_onramping_src.len() {
        return Err(Dis::InconsistentSignable.into());
    }
    let amt = src.iter().zip(aggregated_onramping_src.iter()).try_fold(
        0u64,
        |amt_cumulative, (src, aggregated_onramping_src)| -> Result<_, Error> {
            match src {
                OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr,
                    amt,
                    asset,
                }) => {
                    if exp_asset != *asset {
                        return Err(Dis::InconsistentAsset.into());
                    }
                    if exp_acc != *acc_addr {
                        return Err(Dis::InconsistentAccount.into());
                    }
                    let asset_u: U = asset.addr().into();
                    let amt_u = U::from(*amt);
                    storage::onramped_bal::checked_sub_res(
                        &asset_u,
                        &acc_addr.into(),
                        &amt_u,
                        "onchain pot source",
                    )?;
                    event_emit_bal_update(
                        *acc_addr,
                        asset.addr(),
                        U::ZERO,
                        storage::global_bal::get(&asset_u),
                    );
                    amt_cumulative
                        .checked_add(*amt)
                        .ok_or_else(|| Dis::CheckedAddOverflow.into())
                }
                OnrampingSource::AggregatedResult(AggregatedResultArgs { from, amt_to_take }) => {
                    let source_hash = match from.as_ref() {
                        AggregateEnum::Args(agg) => unsafe {
                            let aggregated_res = aggregated_onramping_src
                                .aggregated_res
                                .as_ref()
                                .ok_or(Dis::InconsistentSignable)?;
                            recurse_aggregate_args(
                                imm,
                                &agg.args,
                                aggregated_res,
                                &agg.server_sig,
                            )?;
                            aggregated_res.hash.clone().into()
                        },
                        AggregateEnum::Hash(h) => *h,
                    };
                    // Both representations consume the same settled ledger. The
                    // nonzero side identifies the prior winner, NOT is_taker.
                    let asset_u = storage::hash_asset::get(&source_hash);
                    let asset: [u8; 20] = asset_u.into();
                    let (available, acc, source_is_taker) =
                        aggregate_hash_winner(&source_hash, &asset_u)?;
                    if exp_asset.addr() != asset {
                        return Err(Dis::InconsistentAsset.into());
                    }
                    if exp_acc != acc {
                        return Err(Dis::InconsistentAccount.into());
                    }
                    let amount_to_onramp = available
                        .checked_sub(*amt_to_take)
                        .ok_or(Dis::CheckedSubUnderflow)?;
                    let asset_u = U::from(asset);
                    if source_is_taker {
                        storage::hash_taker_leftover::set(&source_hash, &asset_u, &U::ZERO);
                    } else {
                        storage::hash_maker_leftover::set(&source_hash, &asset_u, &U::ZERO);
                    }
                    storage::onramped_bal::checked_add_res(
                        &asset_u,
                        &acc.into(),
                        &amount_to_onramp.into(),
                        "aggregated result remainder to onramped bal",
                    )?;
                    event_emit_bal_update(
                        acc,
                        asset,
                        amount_to_onramp.into(),
                        storage::global_bal::get(&asset_u),
                    );
                    amt_cumulative
                        .checked_add(*amt_to_take)
                        .ok_or_else(|| Dis::CheckedAddOverflow.into())
                }
            }
        },
    )?;
    // These fields describe the NEW bundle role; the other role is unset.
    Ok(RecurseAggregateSrc {
        taker_amt: if is_taker { amt } else { 0 },
        maker_amt: if is_taker { 0 } else { amt },
        taker_acc: if is_taker { exp_acc } else { [0; 20] },
        maker_acc: if is_taker { [0; 20] } else { exp_acc },
        asset: exp_asset.addr(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecurseAggregateArgs {
    pub taker_leftover: u64,
    pub maker_leftover: u64,
    pub taker_acc: [u8; 20],
    pub asset: [u8; 20],
    pub maker_acc: [u8; 20],
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
#[bobcat_trace]
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
                                sig: ref taker_sig,
                            },
                        ms_ts: maker_ms_ts,
                        acc: maker_acc,
                    },
                sig: ref maker_sig,
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
) -> Result<RecurseAggregateArgs, Error> {
    if timestamplet_outcome != outcome {
        return Err(Dis::TimestampletInconsistentOutcome.into());
    }
    if timestamplet_time != expiry {
        return Err(Dis::TimestampletInconsistentTime.into());
    }
    if imm.should_server_sig_validate {
        check_server_sig(&signable.hash, &imm.server_key, server_sig.0)?;
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
    let RecurseAggregateSrc {
        taker_amt,
        taker_acc,
        asset,
        ..
    } = unsafe { recurse_aggregate_src(imm, src, &signable.maker.taker_args.src, true)? };
    let RecurseAggregateSrc {
        maker_amt,
        maker_acc: maker_src_acc,
        asset: maker_asset,
        ..
    } = unsafe { recurse_aggregate_src(imm, maker_src, &signable.maker.src, false)? };
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
            maker_sig.clone(),
        )?;
        check_hash_signer(
            taker_acc,
            signable.maker.taker_args.hash.clone(),
            taker_ms_ts,
            taker_sig.clone(),
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
    let winnings = taker_amt
        .checked_add(maker_amt)
        .ok_or(Dis::CheckedAddOverflow)?;
    let (taker_leftover, maker_leftover) = if did_taker_win {
        (winnings, 0)
    } else {
        (0, winnings)
    };
    storage::hash_asset::set(&signable_hash_u, &asset_u);
    if did_taker_win {
        storage::hash_taker_leftover::set(&signable_hash_u, &asset_u, &U::from(taker_leftover));
    } else {
        storage::hash_maker_leftover::checked_add_res(
            &signable_hash_u,
            &asset_u,
            &U::from(maker_leftover),
            "hash maker leftover checked add",
        )?;
    }
    Ok(RecurseAggregateArgs {
        taker_leftover,
        maker_leftover,
        taker_acc,
        maker_acc,
        asset,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryRecurseAggregateEnum {
    pub sender: [u8; 20],
    pub asset: [u8; 20],
    pub taker_leftover: u64,
    pub maker_leftover: u64,
}

// Entry the aggregate recursive function, returning the amount that's
// able to be withdrawn by the party given. Checks the msg_sender.
#[bobcat_trace]
pub fn entry_recurse_aggregate_enum(
    imm: &Imm,
    args: &AggregateEnum,
    is_taker: bool,
) -> Result<EntryRecurseAggregateEnum, Error> {
    let sender = msg_sender();
    match args {
        AggregateEnum::Args(agg) => {
            let signable = SignableAggregateArgs::from(&agg.args);
            let RecurseAggregateArgs {
                taker_leftover,
                maker_leftover,
                taker_acc,
                maker_acc,
                asset,
            } = unsafe { recurse_aggregate_args(imm, &agg.args, &signable, &agg.server_sig)? };
            if is_taker {
                let recipient = get_acc_owner(taker_acc)?;
                if recipient != sender {
                    return Err(Error {
                        dis: Dis::SenderNotTaker,
                        data: Box::new(ErrorData {
                            sender: Some(sender),
                            expected_sender: Some(recipient),
                            ..ErrorData::default()
                        }),
                    });
                }
                storage::hash_taker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok(EntryRecurseAggregateEnum {
                    sender,
                    asset,
                    taker_leftover,
                    maker_leftover: 0,
                })
            } else {
                let recipient = get_acc_owner(maker_acc)?;
                if recipient != sender {
                    return Err(Error {
                        dis: Dis::SenderNotMaker,
                        data: Box::new(ErrorData {
                            sender: Some(sender),
                            expected_sender: Some(recipient),
                            ..ErrorData::default()
                        }),
                    });
                }
                storage::hash_maker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok(EntryRecurseAggregateEnum {
                    sender,
                    asset,
                    taker_leftover: 0,
                    maker_leftover: maker_leftover.into(),
                })
            }
        }
        AggregateEnum::Hash(h) => {
            let asset = storage::hash_asset::get(h);
            let asset_addr = asset.addr();
            if is_taker {
                let taker_leftover = storage::hash_taker_leftover::get(h, &asset);
                let recipient = get_acc_owner(storage::hash_taker_addr::get(h).into())?;
                if recipient != sender {
                    return Err(Error {
                        dis: Dis::SenderNotTaker,
                        data: Box::new(ErrorData {
                            sender: Some(sender),
                            expected_sender: Some(recipient),
                            hash: Some(*h),
                            ..ErrorData::default()
                        }),
                    });
                }
                storage::hash_taker_leftover::set(h, &asset, &U::ZERO);
                Ok(EntryRecurseAggregateEnum {
                    sender,
                    asset: asset_addr,
                    taker_leftover: taker_leftover.into(),
                    maker_leftover: 0,
                })
            } else {
                let recipient = get_acc_owner(storage::hash_maker_addr::get(h).into())?;
                if recipient != sender {
                    return Err(Dis::SenderNotMaker.into());
                }
                let maker_leftover = storage::hash_maker_leftover::get(h, &asset);
                storage::hash_maker_leftover::set(h, &asset, &U::ZERO);
                Ok(EntryRecurseAggregateEnum {
                    sender,
                    asset: asset_addr,
                    taker_leftover: 0,
                    maker_leftover: maker_leftover.into(),
                })
            }
        }
    }
}

#[bobcat_trace]
pub fn entry_aggregate_to_offramp(imm: &Imm, args: &[AggregateToOfframpArg]) -> usize {
    // This code recursively descends inside the aggregate args, finding the
    // correct winning amount for the maker and takers, crediting the winning
    // addresses correctly. The amounts are then sent to the users.
    let mut new_bal = U::ZERO;
    for AggregateToOfframpArg { args, is_taker } in args {
        let EntryRecurseAggregateEnum {
            sender,
            asset,
            taker_leftover,
            maker_leftover,
        } = entry_recurse_aggregate_enum(imm, args, *is_taker).unwrap();
        let hash = match args {
            AggregateEnum::Args(agg) => SignableAggregateArgs::from(&agg.args).hash.into(),
            AggregateEnum::Hash(hash) => *hash,
        };
        let acc_addr: [u8; 20] = if *is_taker {
            storage::hash_taker_addr::get(&hash).into()
        } else {
            storage::hash_maker_addr::get(&hash).into()
        };
        let asset_u = U::from(asset);
        let leftover_u = U::from(taker_leftover) + U::from(maker_leftover);
        new_bal = new_bal.checked_add(&leftover_u);
        let global_bal =
            storage::global_bal::checked_sub_res(&asset_u, &leftover_u, "aggregate offramp")
                .unwrap();
        event_emit_bal_update(acc_addr, asset, U::ZERO, global_bal);
        assert!(
            call_eip20::transfer(asset, sender, &leftover_u),
            "eip20 transfer err"
        );
    }
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
    {
        let hash: U = _hash.into();
        if call_accounts::check_hash_statement(_addr, hash, _ms_ts, &_sig.0) {
            Ok(())
        } else {
            Err(Error {
                dis: Dis::StatementCheckFailed,
                data: Box::new(ErrorData {
                    acc_addr: Some(_addr),
                    acc_ms_ts: Some(_ms_ts),
                    acc_sig: Some(_sig),
                    acc_msg: Some(hash),
                    ..ErrorData::default()
                }),
            })
        }
    }
}

fn check_server_sig(hash: &Hash, key: &U, sig: [u8; 64]) -> Result<(), Error> {
    if let Hash::JustInTime(hash) = hash {
        let is_sig_ok = bobcat_sdk::precompiles::superposition::edphverify_post(*hash, *key, sig);
        if is_sig_ok {
            Ok(())
        } else {
            Err(Error {
                dis: Dis::BadSignature,
                data: Box::new(ErrorData {
                    server_key: Some(key.clone()),
                    ..ErrorData::default()
                }),
            })
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
