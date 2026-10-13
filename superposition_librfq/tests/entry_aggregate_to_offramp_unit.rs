// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use superposition_librfq::{entry_aggregate_to_offramp::*, error::Dis, signable::*, *};

extern crate alloc;

use alloc::boxed::Box;

use bobcat_sdk::{maths::U, storage::flush_guard_fresh};

use bobcat_sdk::host::set_contract_address;

use superposition_librfq::std_eip20;

#[test]
fn test_erik_alex_after_already_same_blob() {
    flush_guard_fresh(|| {
        // In this example, Erik is reinvesting his profits to play the
        // game with Alex again.
        let expiry_1 = 1;
        let expiry_2 = 2;
        let outcome = Outcome::BitcoinPrice;
        let alex_addr = [0u8; 20];
        let erik_addr = [1u8; 20];
        let asset = Asset::USDC;
        let erik_amt_1 = 500;
        storage::onramped_bal::set(&asset.into(), &erik_addr.into(), &erik_amt_1.into());
        storage::onramped_bal::set(&asset.into(), &alex_addr.into(), &U::from_u32(500));
        let erik_1 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: erik_addr,
                    amt: erik_amt_1,
                    asset,
                })],
                expiry: expiry_1,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_1 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: alex_addr,
                    amt: 200,
                    asset,
                })],
                taker: erik_1,
                ms_ts: [1u8; 6],
                acc: alex_addr,
            },
            sig: Sig::default(),
        };
        let agg_1 = AggregateArgs {
            maker: alex_1,
            timestamplet: Timestamplet {
                timestamplet_time: 1,
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };
        let erik_2 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![OnrampingSource::AggregatedResult(AggregatedResultArgs {
                    from: Box::new(AggregateEnum::Args(Box::new(Aggregate {
                        args: agg_1,
                        server_sig: Sig::default(),
                    }))),
                    amt_to_take: 700,
                })],
                expiry: expiry_2,
                ms_ts: [2u8; 6],
                is_up: false,
                price_target: 50,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_2 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: alex_addr,
                    amt: 300,
                    asset,
                })],
                taker: erik_2,
                ms_ts: [3u8; 6],
                acc: alex_addr,
            },
            sig: Sig::default(),
        };
        let agg_2 = AggregateArgs {
            maker: alex_2,
            timestamplet: Timestamplet {
                timestamplet_time: 2,
                timestamplet_outcome: outcome,
                timestamplet_val: 40,
            },
        };
        let signable = SignableAggregateArgs::from(&agg_2);
        assert_eq!((1000, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_2, &signable, &Sig::default()).unwrap()
        });
    })
}

#[test]
fn test_inline_reinvest_consumes_inner_leftover() {
    flush_guard_fresh(|| {
        let taker = [1u8; 20]; // erik
        let maker = [0u8; 20]; // alex
        let asset = Asset::USDC;
        let expiry_1 = 1;
        let expiry_2 = 2;
        let outcome = Outcome::BitcoinPrice;

        // The pool: taker deposits 500 for game 1 plus another 100 that is
        // combined with the recursive result in game 2. The maker stakes
        // 200 (game 1) + 300 (game 2).
        storage::onramped_bal::set(&asset.into(), &taker.into(), &U::from_u32(500 + 100));
        storage::onramped_bal::set(&asset.into(), &maker.into(), &U::from_u32(200 + 300));

        // Game 1: taker is_up=true, target 100, value 101 > 100 → taker wins
        // 500 + 200 = 700.
        let erik_1 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: taker,
                    amt: 500,
                    asset,
                })],
                expiry: expiry_1,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_1 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: maker,
                    amt: 200,
                    asset,
                })],
                taker: erik_1,
                ms_ts: [1u8; 6],
                acc: maker,
            },
            sig: Sig::default(),
        };
        let agg_1 = AggregateArgs {
            maker: alex_1,
            timestamplet: Timestamplet {
                timestamplet_time: expiry_1,
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };

        // Game 2: taker reinvests the 700 inline (not by hash), combines it
        // with a second on-chain source of 100, and wins again:
        // 700 + 100 + 300 = 1100.
        let erik_2 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![
                    OnrampingSource::AggregatedResult(AggregatedResultArgs {
                        from: Box::new(AggregateEnum::Args(Box::new(Aggregate {
                            args: agg_1,
                            server_sig: Sig::default(),
                        }))),
                        amt_to_take: 700,
                    }),
                    OnrampingSource::OnchainPot(OnchainPotArgs {
                        acc_addr: taker,
                        amt: 100,
                        asset,
                    }),
                ],
                expiry: expiry_2,
                ms_ts: [2u8; 6],
                is_up: false,
                price_target: 50,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_2 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: maker,
                    amt: 300,
                    asset,
                })],
                taker: erik_2,
                ms_ts: [3u8; 6],
                acc: maker,
            },
            sig: Sig::default(),
        };
        let agg_2 = AggregateArgs {
            maker: alex_2,
            timestamplet: Timestamplet {
                timestamplet_time: expiry_2,
                timestamplet_outcome: outcome,
                timestamplet_val: 40,
            },
        };

        let signable_2 = SignableAggregateArgs::from(&agg_2);
        assert_eq!((1100, taker, asset.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_2, &signable_2, &Sig::default()).unwrap()
        });

        // The inner (game 1) aggregate's taker claim of 700 was fully
        // reinvested into game 2, so its leftover must now be ZERO. If it is
        // still live, the taker holds a claim of 700 (inner) PLUS 1000 (outer)
        // against a pool of only 1000 → double-withdrawal / fund creation.
        let inner_hash: U = signable_2
            .maker
            .taker_args
            .src
            .first()
            .expect("inline reinvest has one source")
            .aggregated_res
            .as_ref()
            .expect("inline reinvest carries an aggregated_res")
            .hash
            .clone()
            .into();
        let outer_hash: U = signable_2.hash.clone().into();
        assert_eq!(
            storage::hash_taker_leftover::get(&inner_hash, &asset.into()),
            U::ZERO,
            "inline reinvest must consume the inner taker leftover"
        );
        assert_eq!(
            storage::hash_taker_leftover::get(&outer_hash, &asset.into()),
            U::from_u32(1100),
            "the only live taker claim is the outer aggregate's 1100"
        );

        // Conservation: the sum of live taker claims cannot exceed the pool.
        let pool = U::from_u32(500 + 100 + 200 + 300);
        let live_taker_claims = U::ZERO
            .checked_add(&storage::hash_taker_leftover::get(
                &inner_hash,
                &asset.into(),
            ))
            .checked_add(&storage::hash_taker_leftover::get(
                &outer_hash,
                &asset.into(),
            ));
        assert!(
            live_taker_claims <= pool,
            "taker claims {live_taker_claims:?} exceed the pool {pool:?}"
        );
    })
}

#[test]
fn test_multiple_sources_reject_amount_overflow() {
    flush_guard_fresh(|| {
        let account = [7u8; 20];
        let asset = Asset::USDC;
        let balance = U::from(u64::MAX).checked_add(&U::ONE);
        storage::onramped_bal::set(&asset.into(), &account.into(), &balance);

        let src = vec![
            OnrampingSource::OnchainPot(OnchainPotArgs {
                acc_addr: account,
                amt: u64::MAX,
                asset,
            }),
            OnrampingSource::OnchainPot(OnchainPotArgs {
                acc_addr: account,
                amt: 1,
                asset,
            }),
        ];
        let signable = src
            .iter()
            .map(SignableOnrampingSource::from)
            .collect::<Vec<_>>();

        let err = unsafe { recurse_aggregate_src(&Imm::default(), &src, &signable) }
            .expect_err("a source total above u64::MAX must be rejected");
        assert_eq!(Dis::CheckedAddOverflow, err.dis);
    })
}

#[test]
fn test_erik_alex_after_posted_blob() {
    flush_guard_fresh(|| {
        // In this example, Erik is reinvesting profits from a result that was
        // already posted on-chain, so the second aggregate refers to the first
        // aggregate by hash instead of inlining the same blob again.
        let expiry_1 = 1;
        let expiry_2 = 2;
        let outcome = Outcome::BitcoinPrice;
        let alex_addr = [0u8; 20];
        let erik_addr = [1u8; 20];
        let asset = Asset::USDC;
        storage::onramped_bal::set(&asset.into(), &alex_addr.into(), &U::from_u32(200 + 300));
        storage::onramped_bal::set(&asset.into(), &erik_addr.into(), &U::from_u32(500));
        let erik_1 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: erik_addr,
                    amt: 500,
                    asset: Asset::USDC,
                })],
                expiry: expiry_1,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_1 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: alex_addr,
                    amt: 200,
                    asset,
                })],
                taker: erik_1,
                ms_ts: [1u8; 6],
                acc: alex_addr,
            },
            sig: Sig::default(),
        };
        let agg_1 = AggregateArgs {
            maker: alex_1,
            timestamplet: Timestamplet {
                timestamplet_time: 1,
                timestamplet_outcome: outcome,
                timestamplet_val: 101,
            },
        };
        let signable_1 = SignableAggregateArgs::from(&agg_1);
        assert_eq!((700, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_1, &signable_1, &Sig::default()).unwrap()
        });
        let erik_2 = BundleTaker {
            args: BundleTakerArgs {
                src: vec![OnrampingSource::AggregatedResult(AggregatedResultArgs {
                    from: Box::new(AggregateEnum::Hash(signable_1.hash.into())),
                    amt_to_take: 700,
                })],
                expiry: expiry_2,
                ms_ts: [2u8; 6],
                is_up: false,
                price_target: 50,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_2 = BundleMaker {
            args: BundleMakerArgs {
                src: vec![OnrampingSource::OnchainPot(OnchainPotArgs {
                    acc_addr: alex_addr,
                    amt: 300,
                    asset,
                })],
                taker: erik_2,
                ms_ts: [3u8; 6],
                acc: alex_addr,
            },
            sig: Sig::default(),
        };
        let agg_2 = AggregateArgs {
            maker: alex_2,
            timestamplet: Timestamplet {
                timestamplet_time: 2,
                timestamplet_outcome: outcome,
                timestamplet_val: 40,
            },
        };
        let signable_2 = SignableAggregateArgs::from(&agg_2);
        // Referring to the posted aggregate by hash preserves the complete
        // taker return, just like inlining the first aggregate above.
        assert_eq!((1000, erik_addr, Asset::USDC.addr()), unsafe {
            recurse_aggregate_args(&Imm::default(), &agg_2, &signable_2, &Sig::default()).unwrap()
        });
    })
}

// When the taker loses, the maker wins the full pool: the taker's stake
// plus the maker's own principal back. This locks in that a settling maker
// never forfeits their own stake, and that paying them stays solvent
// (the payout is exactly backed by the pool already deducted from the
// participants' onramped balances).
#[test]
fn test_maker_win_offramp_pays_principal() {
    flush_guard_fresh(|| {
        set_contract_address([0x22; 20]);
        let contract = [0x22; 20];
        let taker = [0u8; 20];
        let maker = [1u8; 20];
        let asset = Asset::USDC;
        let asset_addr = asset.addr();
        let taker_amt: u64 = 500;
        let maker_amt: u64 = 200;
        let pool = taker_amt + maker_amt;
        let outcome = Outcome::BitcoinPrice;

        // The contract physically holds every onramped token; the
        // onramped ledger backs the accounts that played the game.
        std_eip20::give(asset_addr, contract, &U::from(pool));
        storage::onramped_bal::set(&asset_addr.into(), &taker.into(), &U::from(taker_amt));
        storage::onramped_bal::set(&asset_addr.into(), &maker.into(), &U::from(maker_amt));
        let contract_before = std_eip20::balance(asset_addr, contract);

        let erik_taker = BundleTaker {
            args: BundleTakerArgs {
                src: vec![
                    OnrampingSource::OnchainPot(OnchainPotArgs {
                        acc_addr: taker,
                        amt: 200,
                        asset,
                    }),
                    OnrampingSource::OnchainPot(OnchainPotArgs {
                        acc_addr: taker,
                        amt: taker_amt - 200,
                        asset,
                    }),
                ],
                expiry: 9,
                ms_ts: [0u8; 6],
                is_up: true,
                price_target: 100,
                outcome,
            },
            sig: Sig::default(),
        };
        let alex_maker = BundleMaker {
            args: BundleMakerArgs {
                src: vec![
                    OnrampingSource::OnchainPot(OnchainPotArgs {
                        acc_addr: maker,
                        amt: 50,
                        asset,
                    }),
                    OnrampingSource::OnchainPot(OnchainPotArgs {
                        acc_addr: maker,
                        amt: maker_amt - 50,
                        asset,
                    }),
                ],
                taker: erik_taker,
                ms_ts: [1u8; 6],
                acc: maker,
            },
            sig: Sig::default(),
        };
        let agg = AggregateArgs {
            maker: alex_maker,
            timestamplet: Timestamplet {
                timestamplet_time: 9,
                timestamplet_outcome: outcome,
                // is_up && val > target → taker would win; 40 > 100 is false, so the
                // taker LOSES and the maker wins.
                timestamplet_val: 40,
            },
        };
        let signable = SignableAggregateArgs::from(&agg);
        assert_eq!(signable.maker.src.len(), 2);
        assert_eq!(signable.maker.taker_args.src.len(), 2);
        assert!(
            signable
                .maker
                .src
                .iter()
                .chain(signable.maker.taker_args.src.iter())
                .all(|src| src.aggregated_res.is_none()),
            "each on-chain source should have a corresponding signable source",
        );
        let (taker_leftover, returned_taker, returned_asset) = unsafe {
            recurse_aggregate_args(&Imm::default(), &agg, &signable, &Sig::default()).unwrap()
        };
        // The loser (taker) gets nothing back.
        assert_eq!(
            (0, taker, asset_addr),
            (taker_leftover, returned_taker, returned_asset)
        );

        let hash: U = signable.hash.clone().into();
        assert_eq!(
            storage::hash_taker_leftover::get(&hash, &asset_addr.into()),
            U::ZERO
        );
        // The maker's claim is the whole pool: winnings + principal.
        assert_eq!(
            storage::hash_maker_leftover::get(&hash, &asset_addr.into()),
            U::from(pool)
        );

        // Offramp the maker's entire claim and check conservation.
        assert!(call_eip20::transfer(asset_addr, maker, &U::from(pool)));
        assert_eq!(std_eip20::balance(asset_addr, maker), U::from(pool));
        assert_eq!(
            std_eip20::balance(asset_addr, contract),
            contract_before.checked_sub(&U::from(pool))
        );

        std_eip20::reset_balances();
    })
}
