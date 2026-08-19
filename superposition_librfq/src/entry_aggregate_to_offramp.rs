use crate::{
    Aggregate, AggregateArgs, AggregateEnum, AggregateToOfframpArgs, BundleMaker, BundleMakerArgs,
    BundleTaker, BundleTakerArgs, Imm, OnchainPotArgs, OnrampingSource, Sig, Timestamplet,
    call_accounts,
    error::{Dis, Error},
    signable::{Hash, SignableAggregateArgs},
    storage,
};

use bobcat_sdk::{entry::msg_sender, maths::U};

// Recursively descend into the aggregated bundle, returning the address
// of the taker, and the amount they provided.
unsafe fn recurse_aggregate_src(
    imm: &Imm,
    src: &OnrampingSource,
    signable: &SignableAggregateArgs,
) -> Result<(u64, [u8; 20], [u8; 20]), Error> {
    match src {
        OnrampingSource::OnchainPot(OnchainPotArgs {
            acc_addr,
            amt,
            asset,
        }) => {
            // TODO: check one last time if we're able to use this by
            // checking if the hash is onchain:
            Ok((*amt, *acc_addr, asset.addr()))
        }
        OnrampingSource::AggregatedResult(r) => {
            match &**r {
                AggregateEnum::Args(Aggregate { args, server_sig }) => unsafe {
                    recurse_aggregate_args(imm, args, signable, server_sig)
                },
                // Since we use the storage to get info on what's available to be
                // spent, this is a noop, and we just return what's there:
                AggregateEnum::Hash(h) => {
                    // We need to zero out the taker's position here, so
                    // that the next code can assume it's unset. The offramp function
                    // for the maker can adjust this itself, since we're not going to assume
                    // that the maker wants to get their money out this way:
                    let asset = storage::hash_asset::get(h);
                    storage::hash_taker_leftover::set(
                        &signable.hash.clone().into(),
                        &asset.into(),
                        &U::ZERO,
                    );
                    Ok((
                        storage::hash_taker_leftover::get(h, &asset).into(),
                        storage::hash_taker_addr::get(h).into(),
                        asset.into(),
                    ))
                }
            }
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
unsafe fn recurse_aggregate_args(
    imm: &Imm,
    &AggregateArgs {
        maker:
            BundleMaker {
                args:
                    BundleMakerArgs {
                        amt: maker_amt,
                        taker:
                            BundleTaker {
                                args:
                                    BundleTakerArgs {
                                        ref src,
                                        expiry,
                                        ms_ts,
                                        is_up,
                                        price_target,
                                        outcome,
                                    },
                                sig: taker_sig,
                            },
                        ms_ts: maker_ms_ts,
                        acc: maker_acc,
                    },
                sig: bundle_sig,
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
    // TODO: descend into the signable here.
    let (taker_amt, taker_acc, asset) = unsafe { recurse_aggregate_src(imm, &src, signable)? };
    let did_taker_win = match (is_up, timestamplet_val > price_target) {
        (true, true) | (false, false) => true,
        _ => false,
    };
    let asset_u = U::from(asset);
    let maker_amt_u = U::from(maker_amt);
    let taker_amt_u = U::from(taker_amt);
    let signable_hash_u = U::from(signable.hash.clone());
    storage::hash_taker_addr::set(&signable_hash_u, &taker_acc.into());
    storage::hash_maker_addr::set(&signable_hash_u, &maker_acc.into());
    if did_taker_win {
        storage::hash_taker_position::checked_add_res(&signable_hash_u, &asset_u, &maker_amt_u)?;
        storage::hash_taker_leftover::checked_add_res(&signable_hash_u, &asset_u, &maker_amt_u)?;
    } else {
        storage::hash_maker_position::checked_add_res(&signable_hash_u, &asset_u, &taker_amt_u)?;
        storage::hash_maker_leftover::checked_add_res(&signable_hash_u, &asset_u, &taker_amt_u)?;
    }
    storage::hash_asset::set(&signable_hash_u, &asset_u);
    let taker_leftover = if did_taker_win {
        taker_amt
            .checked_add(maker_amt)
            .ok_or(Dis::CheckedAddOverflow)
    } else {
        taker_amt
            .checked_sub(taker_amt)
            .ok_or(Dis::CheckedSubUnderflow)
    }?;
    Ok((taker_leftover, taker_acc, asset))
}

// Entry the aggregate recursive function, returning the amount that's
// able to be withdrawn by the party given.
pub fn recurse_aggregate_enum(
    imm: &Imm,
    args: &AggregateEnum,
    is_taker: bool,
    server_sig: Sig,
) -> Result<u64, Error> {
    match args {
        AggregateEnum::Args(Aggregate { args, server_sig }) => {
            let signable = SignableAggregateArgs::from(args);
            let (taker_leftover, taker_acc, asset) =
                unsafe { recurse_aggregate_args(imm, args, &signable, server_sig)? };
            let sender = msg_sender();
            if is_taker {
                let recipient = call_accounts::get_owner(taker_acc);
                if recipient != sender {
                    return Err(Dis::SenderNotTaker.into());
                }
                storage::hash_taker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok(taker_leftover)
            } else {
                let signable_hash_u: U = signable.hash.clone().into();
                let maker_acc: [u8; 20] = storage::hash_maker_addr::get(&signable_hash_u).into();
                let recipient = call_accounts::get_owner(maker_acc);
                if recipient != sender {
                    return Err(Dis::SenderNotTaker.into());
                }
                let maker_leftover =
                    storage::hash_maker_leftover::get(&signable_hash_u, &asset.into());
                storage::hash_maker_leftover::set(&signable.hash.into(), &asset.into(), &U::ZERO);
                Ok(maker_leftover.into())
            }
        }
        AggregateEnum::Hash(h) => {
            let asset = storage::hash_asset::get(h);
            Ok(if is_taker {
                let amt = storage::hash_taker_leftover::get(&h, &asset.into());
                storage::hash_taker_leftover::set(&h, &asset.into(), &U::ZERO);
                amt
            } else {
                let amt = storage::hash_maker_leftover::get(&h, &asset.into());
                storage::hash_maker_leftover::set(&h, &asset.into(), &U::ZERO);
                amt
            }
            .into())
        }
    }
}

pub fn entry_aggregate_to_offramp(
    imm: &Imm,
    AggregateToOfframpArgs { args, is_taker }: AggregateToOfframpArgs,
    server_sig: Sig,
) -> usize {
    // This code recursively descends inside the aggregate args, finding the
    // correct winning amount for the maker and takers, crediting the winning
    // addresses correctly. The amounts are then sent to the users.
    recurse_aggregate_enum(imm, &args, is_taker, server_sig).unwrap();
    0
}

fn check_hash_signer(addr: [u8; 20], hash: Hash, ms_ts: [u8; 6], sig: Sig) -> Result<(), Error> {
    if call_accounts::check_hash_statement(addr, hash, ms_ts, &sig) {
        Ok(())
    } else {
        Err(Dis::StatementCheckFailed.into())
    }
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Asset, Outcome, Timestamplet};
    use alloc::boxed::Box;

    #[test]
    fn test_erik_alex() {
        // Erik is expecting that the price of Bitcoin will go up above $100 by
        // timestamp 1. Alex bets that it won't, providing $200 as the
        // compensation if Erik is correct. As Alex has provided the highest
        // offer at the conclusion of the sealed bid auction, he is chosen. At
        // timestamp 1, Erik is correct. Erik provided $500 for his prediction,
        // so he ends up with $700.
        let expiry: [u8; 6] = [0, 0, 0, 0, 0, 1];
        let outcome = Outcome::BitcoinPrice;
        let alex_addr = [0u8; 20];
        let erik_addr = [1u8; 20];
        let erik = BundleTaker {
            args: BundleTakerArgs {
                src: OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: erik_addr,
                    amt: 500,
                    asset: Asset::USDC,
                }),
                expiry,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: [0u8; 64],
        };
        let alex = BundleMaker {
            args: BundleMakerArgs {
                taker: erik,
                ms_ts: [0u8; 6],
                acc: alex_addr,
                amt: 200,
            },
            sig: [0u8; 64],
        };
        let agg = AggregateArgs {
            maker: alex,
            timestamplet: Timestamplet {
                timestamplet_time: [0, 0, 0, 0, 0, 1],
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };
        let signable = SignableAggregateArgs::from(&agg);
        assert_eq!((700, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg, &signable, &[0u8; 64]).unwrap()
        });
    }

    #[test]
    fn test_erik_alex_after_already_same_blob() {
        // In this example, Erik is reinvesting his profits to play the
        // game with Alex again.
        let expiry_1 = [0, 0, 0, 0, 0, 1];
        let expiry_2 = [0, 0, 0, 0, 0, 2];
        let outcome = Outcome::BitcoinPrice;
        let alex_addr = [0u8; 20];
        let erik_addr = [1u8; 20];
        let erik_1 = BundleTaker {
            args: BundleTakerArgs {
                src: OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: erik_addr,
                    amt: 500,
                    asset: Asset::USDC,
                }),
                expiry: expiry_1,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: [0u8; 64],
        };
        let alex_1 = BundleMaker {
            args: BundleMakerArgs {
                taker: erik_1,
                ms_ts: [0u8; 6],
                acc: alex_addr,
                amt: 200,
            },
            sig: [0u8; 64],
        };
        let agg_1 = AggregateArgs {
            maker: alex_1,
            timestamplet: Timestamplet {
                timestamplet_time: [0, 0, 0, 0, 0, 1],
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };
        let erik_2 = BundleTaker {
            args: BundleTakerArgs {
                src: OnrampingSource::AggregatedResult(Box::new(AggregateEnum::Args(Aggregate {
                    args: agg_1,
                    server_sig: [0u8; 64],
                }))),
                expiry: expiry_2,
                ms_ts: [1u8; 6],
                is_up: false,
                price_target: 50,
                outcome,
            },
            sig: [0u8; 64],
        };
        let alex_2 = BundleMaker {
            args: BundleMakerArgs {
                taker: erik_2,
                ms_ts: [1u8; 6],
                acc: alex_addr,
                amt: 300,
            },
            sig: [0u8; 64],
        };
        let agg_2 = AggregateArgs {
            maker: alex_2,
            timestamplet: Timestamplet {
                timestamplet_time: [0, 0, 0, 0, 0, 2],
                timestamplet_outcome: outcome,
                timestamplet_val: 40,
            },
        };
        let signable = SignableAggregateArgs::from(&agg_2);
        assert_eq!((1000, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_2, &signable, &[0u8; 64]).unwrap()
        });
    }

    #[test]
    fn test_erik_alex_posted_blob() {
        // In this example, Erik is reinvesting his profits to play the
        // game with Alex again. But unlike the previous example, Alex
        // posted his blob, so the interaction was seen on-chain.
        let expiry_1 = [0, 0, 0, 0, 0, 1];
        let expiry_2 = [0, 0, 0, 0, 0, 2];
        let outcome = Outcome::BitcoinPrice;
        let alex_addr = [0u8; 20];
        let erik_addr = [1u8; 20];
        let erik_1 = BundleTaker {
            args: BundleTakerArgs {
                src: OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: erik_addr,
                    amt: 500,
                    asset: Asset::USDC,
                }),
                expiry: expiry_1,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: [0u8; 64],
        };
        let alex_1 = BundleMaker {
            args: BundleMakerArgs {
                taker: erik_1,
                ms_ts: [0u8; 6],
                acc: alex_addr,
                amt: 200,
            },
            sig: [0u8; 64],
        };
        let agg_1 = AggregateArgs {
            maker: alex_1,
            timestamplet: Timestamplet {
                timestamplet_time: [0, 0, 0, 0, 0, 1],
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };
        let erik_2 = BundleTaker {
            args: BundleTakerArgs {
                src: OnrampingSource::AggregatedResult(Box::new(AggregateEnum::Args(Aggregate {
                    args: agg_1,
                    server_sig: [0u8; 64],
                }))),
                expiry: expiry_2,
                ms_ts: [1u8; 6],
                is_up: false,
                price_target: 50,
                outcome,
            },
            sig: [0u8; 64],
        };
        let alex_2 = BundleMaker {
            args: BundleMakerArgs {
                taker: erik_2,
                ms_ts: [1u8; 6],
                acc: alex_addr,
                amt: 300,
            },
            sig: [0u8; 64],
        };
        let agg_2 = AggregateArgs {
            maker: alex_2,
            timestamplet: Timestamplet {
                timestamplet_time: [0, 0, 0, 0, 0, 2],
                timestamplet_outcome: outcome,
                timestamplet_val: 40,
            },
        };
        let signable = SignableAggregateArgs::from(&agg_2);
        assert_eq!((1000, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_2, &signable, &[0u8; 64]).unwrap()
        });
    }
}
