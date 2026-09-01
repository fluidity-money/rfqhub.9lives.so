#[cfg(all(feature = "proptest", feature = "arbitrary", feature = "std"))]
mod test {
    use std::{
        any::Any,
        collections::BTreeMap,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    use proptest::prelude::*;

    use bobcat_sdk::{
        entry::contract_address,
        host::{set_contract_address, set_msg_sender},
        maths::U,
        storage::{const_keccak256, flush_guard_fresh, slot_map, storage_store},
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

    fn panic_message(payload: Box<dyn Any + Send>) -> String {
        match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => match payload.downcast::<&'static str>() {
                Ok(message) => (*message).to_owned(),
                Err(_) => "non-string panic".to_owned(),
            },
        }
    }

    fn aggregate_from_submission(
        submission: &InvertedSubmission,
        source: OnrampingSource,
        asset: Asset,
    ) -> Aggregate {
        Aggregate {
            args: AggregateArgs {
                maker: BundleMaker {
                    args: BundleMakerArgs {
                        src: OnrampingSource::OnchainPot(OnchainPotArgs {
                            acc_addr: submission.maker_acc,
                            amt: submission.maker_amt,
                            asset,
                        }),
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
        fn offramp_blob_requires_the_embedded_accounts_owner(
            asset in any::<Asset>(),
            amt in 1_u64..=u64::MAX,
            ms_ts in any::<[u8; 6]>(),
            acc_addr in any::<[u8; 20]>(),
            owner in any::<[u8; 20]>(),
            unauthorized_submitter in any::<[u8; 20]>(),
        ) {
            prop_assume!(unauthorized_submitter != owner);

            flush_guard_fresh(|| {
                std_eip20::reset_balances();
                set_contract_address(CONTRACT);

                let owner_slot = slot_map(
                    &U::from(acc_addr),
                    &const_keccak256(b"arbos.recipient"),
                );
                storage_store(&owner_slot, &U::from(owner));
                onramp(asset, acc_addr, amt);

                let args = OfframpArgs {
                    asset,
                    amt,
                    ms_ts,
                    acc_addr,
                };
                let asset_addr = asset.addr();

                set_msg_sender(unauthorized_submitter);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    entry_offramp::entry_offramp(&Imm::default(), args.clone(), [0_u8; 64])
                }));

                let error = result.expect_err(
                    "an address other than the account owner submitted the offramp blob",
                );
                assert!(
                    panic_message(error).contains("not owner of account for offramp"),
                    "offramp failed for a reason other than submitter/owner authorization",
                );
                assert_eq!(
                    storage::onramped_bal::get(&asset.u(), &U::from(acc_addr)),
                    U::from(amt),
                    "an unauthorized submission changed the account balance",
                );
                assert_eq!(
                    storage::server_ms_ts::get(&U::from(acc_addr), &U::from(ms_ts)),
                    U::ZERO,
                    "an unauthorized submission consumed the replay key",
                );
                assert_eq!(eip20_balance(asset_addr, owner), U::ZERO);

                set_msg_sender(owner);
                assert_eq!(
                    entry_offramp::entry_offramp(&Imm::default(), args, [0_u8; 64]),
                    0,
                );
                assert_eq!(
                    storage::onramped_bal::get(&asset.u(), &U::from(acc_addr)),
                    U::ZERO,
                );
                assert_eq!(eip20_balance(asset_addr, owner), U::from(amt));
                assert_eq!(eip20_balance(asset_addr, CONTRACT), U::ZERO);

                std_eip20::reset_balances();
            });
        }

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
                    let aggregate = aggregate_from_submission(submission, source, asset);
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
                        source = OnrampingSource::AggregatedResult(AggregatedResultArgs {
                            from: Box::new(AggregateEnum::Hash(posted_hash)),
                            amt_to_take: submission.price_move.expected_return,
                        });
                    } else {
                        // Keep this node inline in the next posted chunk.
                        source = OnrampingSource::AggregatedResult(AggregatedResultArgs {
                            from: Box::new(AggregateEnum::Args(Box::new(aggregate))),
                            amt_to_take: submission.price_move.expected_return,
                        });
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
