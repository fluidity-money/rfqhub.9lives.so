#[cfg(all(feature = "proptest", feature = "arbitrary", feature = "std"))]
mod test {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use bobcat_sdk::{
        entry::contract_address,
        host::{set_contract_address, set_msg_sender},
        maths::U,
        storage::flush_guard_fresh,
    };

    use superposition_librfq::{
        entry_aggregate_to_offramp::recurse_aggregate_args, signable::SignableAggregateArgs,
        strategies::*, *,
    };

    const CONTRACT: [u8; 20] = [0x11; 20];

    fn eip20_balance(asset: [u8; 20], owner: [u8; 20]) -> U {
        std_eip20::BALANCES.with(|balances| {
            balances
                .borrow()
                .get(&asset)
                .and_then(|owners| owners.get(&owner))
                .copied()
                .unwrap_or(U::ZERO)
        })
    }

    fn aggregate_from_submission(
        submission: &InvertedSubmission,
        source: OnrampingSource,
    ) -> Aggregate {
        Aggregate {
            args: AggregateArgs {
                maker: BundleMaker {
                    args: BundleMakerArgs {
                        amt: submission.maker_amt,
                        taker: BundleTaker {
                            args: BundleTakerArgs {
                                src: source,
                                expiry: submission.expiry,
                                ms_ts: submission.taker_ms_ts,
                                is_up: submission.price_move.direction.is_up(),
                                price_target: submission.price_target,
                                outcome: submission.outcome,
                            },
                            sig: submission.taker_sig,
                        },
                        ms_ts: submission.maker_ms_ts,
                        acc: submission.maker_acc,
                    },
                    sig: submission.maker_sig,
                },
                timestamplet: Timestamplet {
                    timestamplet_time: submission.expiry,
                    timestamplet_outcome: submission.outcome,
                    timestamplet_val: submission.timestamplet_val,
                },
            },
            server_sig: submission.server_sig,
        }
    }

    fn onramp(asset: Asset, account: [u8; 20], amount: u64) {
        let asset_addr = asset.addr();
        std_eip20::give(asset_addr, account, &amount.into());
        set_msg_sender(account);
        entry_onramping_from_bal::entry_onramping_from_bal(asset, amount);
        assert_eq!(eip20_balance(asset_addr, account), U::ZERO);
    }

    proptest! {
        #[test]
        fn test_extended_form(
            inverted in any::<InvertedAggregate>(),
            calldata_boundaries in prop::collection::vec(any::<bool>(), 0..8),
        ) {
            flush_guard_fresh(|| {
                std_eip20::reset_balances();
                set_contract_address(CONTRACT);
                assert_eq!(contract_address(), CONTRACT);

                let asset = inverted.initial_taker.asset;
                let asset_addr = asset.addr();
                let submissions: Vec<_> = inverted.submissions().cloned().collect();

                // Give every participant enough EIP-20 balance, then use the real
                // onramping entry point to move it into the contract and its ledger.
                let mut deposits = BTreeMap::<[u8; 20], u64>::new();
                deposits.insert(inverted.initial_taker.acc_addr, inverted.initial_taker.amt);
                for submission in &submissions {
                    let balance = deposits.entry(submission.maker_acc).or_default();
                    *balance = balance.checked_add(submission.maker_amt).unwrap();
                }
                let total_deposited = deposits.values().copied().sum::<u64>();
                for (account, amount) in deposits {
                    onramp(asset, account, amount);
                }
                assert_eq!(
                    eip20_balance(asset_addr, CONTRACT),
                    U::from(total_deposited)
                );

                let mut source = OnrampingSource::OnchainPot(inverted.initial_taker.clone());
                let mut latest_hash = None;
                let mut final_result = None;

                for (index, submission) in submissions.iter().enumerate() {
                    let aggregate = aggregate_from_submission(submission, source);
                    let is_last = index + 1 == submissions.len();
                    // Always post the first game separately when possible, ensuring
                    // later chunks exercise an on-chain hash. Remaining boundaries
                    // are randomized, producing both single-node and nested calldata.
                    let should_post = is_last
                        || (index == 0 && !is_last)
                        || calldata_boundaries.get(index).copied().unwrap_or(false);

                    if should_post {
                        let signable = SignableAggregateArgs::from(&aggregate.args);
                        let posted_hash: U = signable.hash.clone().into();
                        let result = unsafe {
                            recurse_aggregate_args(
                                &Imm::default(),
                                &aggregate.args,
                                &signable,
                                &aggregate.server_sig,
                            )
                            .unwrap()
                        };

                        assert_eq!(result.0, submission.price_move.expected_return);
                        assert_eq!(result.1, inverted.initial_taker.acc_addr);
                        assert_eq!(result.2, asset_addr);
                        if let Some(previous_hash) = latest_hash {
                            assert_eq!(
                                storage::hash_taker_leftover::get(
                                    &previous_hash,
                                    &U::from(asset_addr),
                                ),
                                U::ZERO
                            );
                        }
                        assert_eq!(storage::hash_asset::get(&posted_hash), U::from(asset_addr));
                        assert_eq!(
                            storage::hash_taker_leftover::get(&posted_hash, &U::from(asset_addr)),
                            U::from(submission.price_move.expected_return)
                        );

                        latest_hash = Some(posted_hash);
                        final_result = Some(result);
                        source = OnrampingSource::AggregatedResult(Box::new(
                            AggregateEnum::Hash(posted_hash),
                        ));
                    } else {
                        // Keep this node inline in the next posted chunk.
                        source = OnrampingSource::AggregatedResult(Box::new(
                            AggregateEnum::Args(aggregate),
                        ));
                    }
                }

                let final_hash = latest_hash.expect("the final chunk is always posted");
                let (amount, taker, result_asset) = final_result.unwrap();
                assert_eq!(result_asset, asset_addr);

                // Model the final user offramp through the std EIP-20 implementation.
                storage::hash_taker_leftover::set(
                    &final_hash,
                    &U::from(asset_addr),
                    &U::ZERO,
                );
                assert!(call_eip20::transfer(asset_addr, taker, &U::from(amount)));
                assert_eq!(eip20_balance(asset_addr, taker), U::from(amount));
                assert_eq!(
                    eip20_balance(asset_addr, CONTRACT),
                    U::from(total_deposited - amount)
                );

                std_eip20::reset_balances();
            });
        }
    }
}
