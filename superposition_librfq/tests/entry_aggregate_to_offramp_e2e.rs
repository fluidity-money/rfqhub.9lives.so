// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

#[cfg(all(feature = "proptest", feature = "arbitrary"))]
mod test {
    use std::collections::BTreeMap;

    #[cfg(feature = "arbos")]
    use std::{
        any::Any,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    use proptest::prelude::*;

    use bobcat_sdk::{
        entry::contract_address,
        host::{set_contract_address, set_msg_sender},
        maths::U,
        storage::flush_guard_fresh,
    };

    #[cfg(feature = "arbos")]
    use bobcat_sdk::storage::{const_keccak256, slot_map, storage_store};

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

    #[cfg(feature = "arbos")]
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
                        src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                            acc_addr: submission.maker_acc,
                            amt: submission.maker_amt,
                            asset,
                        })],
                        taker: BundleTaker {
                            args: BundleTakerArgs {
                                src: vec![source],
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

    #[cfg(feature = "arbos")]
    fn set_owner(account: [u8; 20], owner: [u8; 20]) {
        storage_store(
            &slot_map(&U::from(account), &const_keccak256(b"arbos.recipient")),
            &U::from(owner),
        );
    }

    fn pot(account: [u8; 20], amount: u64, asset: Asset) -> OnrampingSource {
        OnrampingSource::OnchainPot(OnchainPotArgs {
            acc_addr: account,
            amt: amount,
            asset,
        })
    }

    fn reuse(from: AggregateEnum, amount: u64) -> OnrampingSource {
        OnrampingSource::AggregatedResult(AggregatedResultArgs {
            from: Box::new(from),
            amt_to_take: amount,
        })
    }

    fn game(
        taker_src: OnrampingSource,
        maker_src: OnrampingSource,
        maker: [u8; 20],
        taker_wins: bool,
        generation: u8,
    ) -> Aggregate {
        Aggregate {
            args: AggregateArgs {
                maker: BundleMaker {
                    args: BundleMakerArgs {
                        src: vec![maker_src],
                        taker: BundleTaker {
                            args: BundleTakerArgs {
                                src: vec![taker_src],
                                expiry: u32::from(generation) * 10,
                                ms_ts: [generation; 6],
                                is_up: true,
                                price_target: 100,
                                outcome: Outcome::BitcoinPrice,
                            },
                            sig: Sig::default(),
                        },
                        ms_ts: [generation; 6],
                        acc: maker,
                    },
                    sig: Sig::default(),
                },
                timestamplet: Timestamplet {
                    timestamplet_time: u32::from(generation) * 10,
                    timestamplet_outcome: Outcome::BitcoinPrice,
                    // Equality deliberately exercises the maker-winning boundary.
                    timestamplet_val: if taker_wins { 101 } else { 100 },
                },
            },
            server_sig: Sig::default(),
        }
    }

    fn hash(aggregate: &Aggregate) -> U {
        SignableAggregateArgs::from(&aggregate.args).hash.into()
    }

    fn settle(aggregate: &Aggregate) -> entry_aggregate_to_offramp::RecurseAggregateArgs {
        unsafe {
            recurse_aggregate_args(
                &Imm::default(),
                &aggregate.args,
                &SignableAggregateArgs::from(&aggregate.args),
                &aggregate.server_sig,
            )
            .unwrap()
        }
    }

    fn assert_leftovers(hash: U, asset: Asset, taker: u64, maker: u64) {
        assert_eq!(
            storage::hash_taker_leftover::get(&hash, &asset.u()),
            U::from(taker)
        );
        assert_eq!(
            storage::hash_maker_leftover::get(&hash, &asset.u()),
            U::from(maker)
        );
    }

    #[test]
    fn all_winner_to_role_transitions_args_hash_and_nested_generations() {
        transition_matrix(false);
    }

    // Bobcat's current std host has chain ID 0 and no chain-ID setter. Keep
    // real withdrawal coverage explicit, but do not confuse it with native
    // settlement coverage or pretend account-contract calls are emulated.
    #[cfg(feature = "arbos")]
    #[test]
    #[ignore = "requires a host with chain ID 31337 for ArbOS recipient hints"]
    fn transition_matrix_through_real_offramp() {
        transition_matrix(true);
    }

    // Uses real onramping and recursive resolution; signatures are disabled.
    fn transition_matrix(withdraw: bool) {
        for asset in [Asset::USDC, Asset::ARB, Asset::WETH] {
            for prior_taker_wins in [false, true] {
                for new_is_taker in [false, true] {
                    for by_hash in [false, true] {
                        for take in [17, 50] {
                            flush_guard_fresh(|| {
                                std_eip20::reset_balances();
                                set_contract_address(CONTRACT);
                                let (a, b, c, d) = ([0x21; 20], [0x22; 20], [0x23; 20], [0x24; 20]);
                                let owner = [0x31; 20];
                                let winner = if prior_taker_wins { a } else { b };
                                for (account, amount) in [(a, 30), (b, 20), (c, 7), (d, 9)] {
                                    onramp(asset, account, amount);
                                }
                                #[cfg(feature = "arbos")]
                                set_owner(winner, owner);
                                let first = game(
                                    pot(a, 30, asset),
                                    pot(b, 20, asset),
                                    b,
                                    prior_taker_wins,
                                    1,
                                );
                                let first_hash = hash(&first);
                                let from = if by_hash {
                                    let result = settle(&first);
                                    assert_eq!(result.taker_acc, a);
                                    assert_eq!(result.maker_acc, b);
                                    assert_leftovers(
                                        first_hash,
                                        asset,
                                        if prior_taker_wins { 50 } else { 0 },
                                        if prior_taker_wins { 0 } else { 50 },
                                    );
                                    AggregateEnum::Hash(first_hash)
                                } else {
                                    AggregateEnum::Args(Box::new(first.clone()))
                                };
                                let source = reuse(from.clone(), take);
                                let second = if new_is_taker {
                                    game(source, pot(c, 7, asset), c, true, 2)
                                } else {
                                    game(pot(c, 7, asset), source, winner, false, 2)
                                };
                                let second_hash = hash(&second);
                                // Hash mode posts an intermediate generation. Args mode
                                // leaves both earlier generations nested in one call.
                                let second_from = if by_hash {
                                    let result = settle(&second);
                                    assert_eq!(
                                        result.taker_acc,
                                        if new_is_taker { winner } else { c }
                                    );
                                    assert_eq!(
                                        result.maker_acc,
                                        if new_is_taker { c } else { winner }
                                    );
                                    assert_leftovers(
                                        second_hash,
                                        asset,
                                        if new_is_taker { take + 7 } else { 0 },
                                        if new_is_taker { 0 } else { take + 7 },
                                    );
                                    AggregateEnum::Hash(second_hash)
                                } else {
                                    AggregateEnum::Args(Box::new(second.clone()))
                                };
                                // Switch roles once more, retain the same winning account,
                                // and leave a second nonzero remainder in its pot.
                                let third = if new_is_taker {
                                    game(
                                        pot(d, 9, asset),
                                        reuse(second_from, take),
                                        winner,
                                        false,
                                        3,
                                    )
                                } else {
                                    game(reuse(second_from, take), pot(d, 9, asset), d, true, 3)
                                };
                                let third_hash = hash(&third);
                                if withdraw {
                                    #[cfg(feature = "arbos")]
                                    {
                                        set_msg_sender(owner);
                                        entry_aggregate_to_offramp::entry_aggregate_to_offramp(
                                            &Imm::default(),
                                            AggregateEnum::Args(Box::new(third)),
                                            !new_is_taker,
                                        );
                                    }
                                    #[cfg(not(feature = "arbos"))]
                                    panic!("withdrawal requires the ArbOS owner backend");
                                } else {
                                    let result = settle(&third);
                                    assert_eq!(result.asset, asset.addr());
                                    assert_eq!(
                                        result.taker_acc,
                                        if new_is_taker { d } else { winner }
                                    );
                                    assert_eq!(
                                        result.maker_acc,
                                        if new_is_taker { winner } else { d }
                                    );
                                    assert_eq!(
                                        result.taker_leftover,
                                        if new_is_taker { 0 } else { take + 9 }
                                    );
                                    assert_eq!(
                                        result.maker_leftover,
                                        if new_is_taker { take + 9 } else { 0 }
                                    );
                                }
                                for h in [first_hash, second_hash] {
                                    assert_leftovers(h, asset, 0, 0);
                                }
                                assert_leftovers(
                                    third_hash,
                                    asset,
                                    if withdraw || new_is_taker {
                                        0
                                    } else {
                                        take + 9
                                    },
                                    if withdraw || !new_is_taker {
                                        0
                                    } else {
                                        take + 9
                                    },
                                );
                                let remainder = 50 - take + 7;
                                for account in [a, b, c, d] {
                                    assert_eq!(
                                        storage::onramped_bal::get(&asset.u(), &account.into()),
                                        U::from(if account == winner { remainder } else { 0 })
                                    );
                                }
                                let paid = if withdraw { take + 9 } else { 0 };
                                assert_eq!(eip20_balance(asset.addr(), owner), U::from(paid));
                                assert_eq!(
                                    eip20_balance(asset.addr(), CONTRACT),
                                    U::from(66 - paid)
                                );
                                assert_eq!(
                                    storage::global_bal::get(&asset.u()),
                                    U::from(66 - paid)
                                );
                                assert_eq!(remainder + take + 9, 66);
                                // A consumed source cannot be used again, even for zero.
                                // Full Args fail the seen-hash check; Hash fails winner lookup.
                                for amount in [0, take] {
                                    let replay = game(
                                        reuse(from.clone(), amount),
                                        pot(c, 0, asset),
                                        c,
                                        true,
                                        4,
                                    );
                                    let error = unsafe {
                                        recurse_aggregate_args(
                                            &Imm::default(),
                                            &replay.args,
                                            &SignableAggregateArgs::from(&replay.args),
                                            &replay.server_sig,
                                        )
                                    }
                                    .unwrap_err();
                                    assert_eq!(
                                        error.dis,
                                        if by_hash {
                                            error::Dis::CheckedSubUnderflow
                                        } else {
                                            error::Dis::HashAlreadySeen
                                        }
                                    );
                                    assert_eq!(
                                        storage::onramped_bal::get(&asset.u(), &winner.into()),
                                        U::from(remainder)
                                    );
                                    assert_eq!(eip20_balance(asset.addr(), owner), U::from(paid));
                                }
                                std_eip20::reset_balances();
                            });
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn recursive_sources_reject_mixed_owner_or_asset() {
        for by_hash in [false, true] {
            for wrong_asset in [false, true] {
                for prior_taker_wins in [false, true] {
                    flush_guard_fresh(|| {
                        std_eip20::reset_balances();
                        set_contract_address(CONTRACT);
                        let (a, b, c) = ([0x21; 20], [0x22; 20], [0x23; 20]);
                        onramp(Asset::USDC, a, 30);
                        onramp(Asset::USDC, b, 20);
                        let first = game(
                            pot(a, 30, Asset::USDC),
                            pot(b, 20, Asset::USDC),
                            b,
                            prior_taker_wins,
                            1,
                        );
                        let winner = if prior_taker_wins { a } else { b };
                        let from = if by_hash {
                            settle(&first);
                            AggregateEnum::Hash(hash(&first))
                        } else {
                            AggregateEnum::Args(Box::new(first))
                        };
                        let mut next = game(
                            pot(
                                if wrong_asset { winner } else { c },
                                0,
                                if wrong_asset { Asset::ARB } else { Asset::USDC },
                            ),
                            pot(c, 0, Asset::USDC),
                            c,
                            true,
                            2,
                        );
                        next.args.maker.args.taker.args.src.push(reuse(from, 17));
                        let error = unsafe {
                            recurse_aggregate_args(
                                &Imm::default(),
                                &next.args,
                                &SignableAggregateArgs::from(&next.args),
                                &next.server_sig,
                            )
                        }
                        .unwrap_err();
                        assert_eq!(
                            error.dis,
                            if wrong_asset {
                                error::Dis::InconsistentAsset
                            } else {
                                error::Dis::InconsistentAccount
                            }
                        );
                        // Host Err does not roll back earlier recursive writes. Each
                        // rejection has fresh storage; this is not an atomicity test.
                        assert_eq!(eip20_balance(Asset::USDC.addr(), CONTRACT), U::from(50));
                        std_eip20::reset_balances();
                    });
                }
            }
        }
    }

    #[cfg(feature = "arbos")]
    #[test]
    #[ignore = "requires a host with chain ID 31337 for ArbOS recipient hints"]
    fn settled_winnings_require_selected_accounts_owner() {
        for taker_wins in [false, true] {
            flush_guard_fresh(|| {
                std_eip20::reset_balances();
                set_contract_address(CONTRACT);
                let (a, b, owner) = ([0x21; 20], [0x22; 20], [0x31; 20]);
                onramp(Asset::USDC, a, 30);
                onramp(Asset::USDC, b, 20);
                set_owner(a, if taker_wins { owner } else { [0x32; 20] });
                set_owner(b, if taker_wins { [0x32; 20] } else { owner });
                let first = game(
                    pot(a, 30, Asset::USDC),
                    pot(b, 20, Asset::USDC),
                    b,
                    taker_wins,
                    1,
                );
                settle(&first);
                let from = AggregateEnum::Hash(hash(&first));
                set_msg_sender([0x32; 20]);
                let error = entry_aggregate_to_offramp::entry_recurse_aggregate_enum(
                    &Imm::default(),
                    &from,
                    taker_wins,
                )
                .unwrap_err();
                assert_eq!(
                    error.dis,
                    if taker_wins {
                        error::Dis::SenderNotTaker
                    } else {
                        error::Dis::SenderNotMaker
                    }
                );
                assert_leftovers(
                    hash(&first),
                    Asset::USDC,
                    if taker_wins { 50 } else { 0 },
                    if taker_wins { 0 } else { 50 },
                );
                set_msg_sender(owner);
                entry_aggregate_to_offramp::entry_aggregate_to_offramp(
                    &Imm::default(),
                    from,
                    taker_wins,
                );
                assert_eq!(eip20_balance(Asset::USDC.addr(), owner), U::from(50));
                assert_eq!(storage::global_bal::get(&Asset::USDC.u()), U::ZERO);
                assert_leftovers(hash(&first), Asset::USDC, 0, 0);
                std_eip20::reset_balances();
            });
        }
    }

    proptest! {
        // This fixture writes the ArbOS recipient-hint slot directly. The non-ArbOS
        // backend resolves owners by calling the account contract, which Bobcat's
        // std host does not emulate.
        #[cfg(feature = "arbos")]
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
                    entry_offramp::entry_offramp(&Imm::default(), args.clone(), Sig::default())
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
                    entry_offramp::entry_offramp(&Imm::default(), args, Sig::default()),
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
                for (&account, &amount) in &deposits {
                    onramp(asset, account, amount);
                }
                assert_eq!(
                    eip20_balance(asset_addr, CONTRACT),
                    U::from(total_deposited)
                );

                let mut source = OnrampingSource::OnchainPot(inverted.initial_taker.clone());
                let mut latest_hash = None;
                let mut final_result = None;
                let mut expected_onramped = deposits;
                let mut next_unsettled = 0;

                for (index, submission) in submissions.iter().enumerate() {
                    // The legacy strategy sizes the next stake from the prior taker
                    // return (possibly zero), but the source belongs to the actual
                    // prior winner, which can now be a maker.
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

                        assert_eq!(result.taker_leftover, submission.price_move.expected_return);
                        let expected_taker = submissions[..index].iter().fold(
                            inverted.initial_taker.acc_addr,
                            |owner, prior| if prior.price_move.expected_return > 0 {
                                owner
                            } else {
                                prior.maker_acc
                            },
                        );
                        assert_eq!(result.taker_acc, expected_taker);
                        assert_eq!(result.maker_acc, submission.maker_acc);
                        assert_eq!(result.asset, asset_addr);
                        assert_eq!(result.taker_leftover + result.maker_leftover,
                            submission.taker_amt + submission.maker_amt);
                        if let Some(previous_hash) = latest_hash {
                            assert_eq!(
                                storage::hash_taker_leftover::get(
                                    &previous_hash,
                                    &U::from(asset_addr),
                                ),
                                U::ZERO
                            );
                            assert_eq!(storage::hash_maker_leftover::get(
                                &previous_hash, &asset.u()), U::ZERO);
                        }
                        assert_eq!(storage::hash_asset::get(&posted_hash), U::from(asset_addr));
                        assert_eq!(storage::hash_maker_leftover::get(&posted_hash, &asset.u()),
                            U::from(result.maker_leftover));
                        assert_eq!(
                            storage::hash_taker_leftover::get(&posted_hash, &U::from(asset_addr)),
                            U::from(submission.price_move.expected_return)
                        );

                        // Apply each newly settled node to an independent ledger model.
                        for settled_index in next_unsettled..=index {
                            let settled = &submissions[settled_index];
                            *expected_onramped.get_mut(&settled.maker_acc).unwrap() -=
                                settled.maker_amt;
                            if settled_index == 0 {
                                *expected_onramped
                                    .get_mut(&inverted.initial_taker.acc_addr)
                                    .unwrap() -= inverted.initial_taker.amt;
                            } else {
                                let prior = &submissions[settled_index - 1];
                                let prior_return = prior.taker_amt + prior.maker_amt;
                                let prior_owner = submissions[..settled_index].iter().fold(
                                    inverted.initial_taker.acc_addr,
                                    |owner, node| if node.price_move.expected_return > 0 {
                                        owner
                                    } else {
                                        node.maker_acc
                                    },
                                );
                                *expected_onramped.get_mut(&prior_owner).unwrap() +=
                                    prior_return - settled.taker_amt;
                            }
                        }
                        next_unsettled = index + 1;
                        for (&account, &expected) in &expected_onramped {
                            assert_eq!(
                                storage::onramped_bal::get(&asset.u(), &U::from(account)),
                                U::from(expected),
                                "onramped balance differs for account {account:?}",
                            );
                        }

                        latest_hash = Some(posted_hash);
                        final_result = Some(result);
                        let amt_to_take = submissions
                            .get(index + 1)
                            .map_or(submission.price_move.expected_return, |next| next.taker_amt);
                        source = OnrampingSource::AggregatedResult(AggregatedResultArgs {
                            from: Box::new(AggregateEnum::Hash(posted_hash)),
                            amt_to_take,
                        });
                    } else {
                        // Keep this node inline in the next posted chunk.
                        let amt_to_take = submissions
                            .get(index + 1)
                            .map_or(submission.price_move.expected_return, |next| next.taker_amt);
                        source = OnrampingSource::AggregatedResult(AggregatedResultArgs {
                            from: Box::new(AggregateEnum::Args(Box::new(aggregate))),
                            amt_to_take,
                        });
                    }
                }

                let final_hash = latest_hash.expect("the final chunk is always posted");
                let result = final_result.unwrap();
                assert_eq!(result.asset, asset_addr);
                let amount = result.taker_leftover + result.maker_leftover;
                let pot_total = expected_onramped.values().copied().sum::<u64>();
                assert_eq!(pot_total + amount, total_deposited);
                assert_eq!(storage::global_bal::get(&asset.u()), U::from(total_deposited));

                // No synthetic transfer: native settlement leaves the final
                // claim in escrow. The ignored ArbOS matrix covers real withdrawal
                // once its host can satisfy the recipient-hint chain-ID guard.
                assert_leftovers(final_hash, asset, result.taker_leftover, result.maker_leftover);
                assert_eq!(eip20_balance(asset_addr, CONTRACT), U::from(total_deposited));

                std_eip20::reset_balances();
            });
        }
    }
}
