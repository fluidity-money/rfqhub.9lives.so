use crate::{
    Aggregate, AggregateArgs, AggregateEnum, Asset, BundleMaker, BundleMakerArgs, BundleTaker,
    BundleTakerArgs, BundleTakerEnum, Imm, OnchainPotArgs, OnrampingSource, Outcome, Sig,
    Timestamplet, call_accounts,
    error::{Dis, Error},
    signable::{Hash, SignableAggregateArgs},
    storage,
};

use bobcat_sdk::maths::U;

#[derive(Debug, Clone)]
struct BundleTakerDetails {
    taker_acc: [u8; 20],
    amt: u64,
    expiry: [u8; 6],
    ms_ts: [u8; 6],
    asset: Asset,
    is_up: bool,
    price_target: u64,
    outcome: Outcome,
}

fn taker_details_from_hash(h: &U) -> Result<BundleTakerDetails, Error> {
    let taker_acc: [u8; 20] = storage::hash_taker_addr::get(&h).into();
    let storage::WordBundleTakerDetails {
        amt,
        expiry,
        ms_ts,
        asset,
        price_target,
        is_up,
        outcome,
    } = storage::hash_taker_details::get(h).try_into()?;
    Ok(BundleTakerDetails {
        taker_acc,
        amt,
        expiry,
        ms_ts,
        asset,
        is_up,
        price_target,
        outcome,
    })
}

// This code recurses into the structure to get the taker's address and
// the taker's details. It will also validate the signature in the signable
// blob that we use for the aggregation function that will be invoking
// this function.
// SAFETY: This code will mutate local storage to move around amounts,
// making it unsafe to be invoked more than once. Corecursive with
// recurse_aggregate.
unsafe fn taker_addr_amt_bundle_taker(
    imm: &Imm,
    x: &BundleTakerEnum,
    signable: &SignableAggregateArgs,
) -> Result<BundleTakerDetails, Error> {
    match x {
        // We have everything in hand, this is a simple case of a single RFQ then offramp:
        BundleTakerEnum::Args(BundleTaker {
            args:
                BundleTakerArgs {
                    src:
                        OnrampingSource::OnchainPot(OnchainPotArgs {
                            acc_addr: taker_acc,
                            amt,
                        }),
                    expiry,
                    ms_ts,
                    asset,
                    is_up,
                    price_target,
                    outcome,
                },
            sig: taker_sig,
        }) => {
            if imm.should_user_sig_validate {
                let hash: Hash = signable.hash.clone();
                check_hash_signer(*taker_acc, hash, *ms_ts, *taker_sig)?;
            }
            Ok(BundleTakerDetails {
                taker_acc: *taker_acc,
                amt: *amt,
                expiry: *expiry,
                ms_ts: *ms_ts,
                asset: *asset,
                is_up: *is_up,
                price_target: *price_target,
                outcome: *outcome,
            })
        }
        // We need to recurse again before bubbling up the state of the
        // user. The next level needs to validate the signature.
        BundleTakerEnum::Args(BundleTaker {
            args:
                BundleTakerArgs {
                    src: OnrampingSource::AggregatedResult(r),
                    ms_ts,
                    ..
                },
            sig: taker_sig,
        }) => {
            // We need to get the hash after creating some side effects
            // here so we can safely look up the user's taker address
            // from the hash:
            match r.as_ref() {
                AggregateEnum::Args(Aggregate { args, server_sig }) => {
                    if imm.should_user_sig_validate {
                        let hash = signable.hash.clone();
                        let taker_acc: [u8; 20] = get_hash_taker(hash.clone());
                        check_hash_signer(taker_acc, hash, *ms_ts, *taker_sig)?;
                    }
                    unsafe { recurse_aggregate(imm, args, &signable, *server_sig) }
                }
                AggregateEnum::Hash(h) => taker_details_from_hash(h),
            }
        }
        // We've already seen all of this and we need to just get the information in the hash:
        BundleTakerEnum::Hash(h) => taker_details_from_hash(h),
    }
}

// Recursively descend into the aggregated bundle, updating the storage
// amounts as we go. The caller should check the storage of the users
// they're draining the liquidity from. This function will zero out the
// amounts left over in the slots as it goes.
// Safety: Uses the contract storage to move amounts around, taking
// advantage of the memory performing caching at places.
unsafe fn recurse_aggregate(
    imm: &Imm,
    args: &AggregateArgs,
    signable: &SignableAggregateArgs,
    server_sig: Sig,
) -> Result<BundleTakerDetails, Error> {
    if imm.should_server_sig_validate {
        check_server_sig(&signable.hash, &imm.server_key, server_sig)?;
    }
    let AggregateArgs {
        maker:
            BundleMaker {
                args:
                    BundleMakerArgs {
                        taker,
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
    } = args;
    // This function also performs some validation of the signature that
    // we use for the server aggregation:
    let BundleTakerDetails {
        taker_acc,
        amt,
        expiry,
        ms_ts: taker_ms_ts,
        asset,
        is_up,
        price_target,
        outcome,
    } = unsafe { taker_addr_amt_bundle_taker(imm, &taker, signable)? };
    // Let's perform some validation here to make sure that everything has
    // worked okay:
    if timestamplet_outcome != &outcome {
        return Err(Dis::TimestampletInconsistentOutcome.into());
    }
    if timestamplet_time != &expiry {
        return Err(Dis::TimestampletInconsistentTime.into());
    }
    // We don't need to validate the Timestamplet, since the
    // AggregateArgs bundle includes it, and we validated the signature
    // there. The taker won if the asset exceeded the price target and
    // they were taking the up position. If it didn't, and it was down,
    // then they lost:
    let did_taker_win = match (timestamplet_val > &price_target, is_up) {
        (true, true) | (false, false) => true,
        _ => false,
    };
    // Now we need to set the fields for the field here:
    unimplemented!()
}

pub fn entry_aggregate_to_offramp(imm: &Imm, args: AggregateArgs, server_sig: Sig) -> usize {
    // This code recursively descends inside the aggregate args, finding the
    // correct winning amount for the maker and takers, crediting the winning
    // addresses correctly. The amounts are then sent to the users.
    // First, we need to validate that this combination is legitimate:
    let signable = SignableAggregateArgs::from(&args);
    unsafe {
        recurse_aggregate(imm, &args, &signable, server_sig).unwrap();
    }
    0
}

fn check_hash_signer(addr: [u8; 20], hash: Hash, ms_ts: [u8; 6], sig: Sig) -> Result<(), Error> {
    if call_accounts::check_hash_statement(addr, hash, ms_ts, &sig) {
        Ok(())
    } else {
        Err(Dis::StatementCheckFailed.into())
    }
}

fn get_hash_taker(hash: Hash) -> [u8; 20] {
    let h: U = hash.into();
    storage::hash_taker_addr::get(&h).into()
}

fn check_server_sig(hash: &Hash, key: &U, sig: [u8; 64]) -> Result<(), Error> {
    if let Hash::JustInTime(hash) = hash {
        if bobcat_sdk::precompiles::superposition::edphverify_post(hash.clone(), key.clone(), sig) {
            Ok(())
        } else {
            Err(Dis::BadSignature.into())
        }
    } else {
        // The hash is already on-chain, we don't need to verify it.
        Ok(())
    }
}
