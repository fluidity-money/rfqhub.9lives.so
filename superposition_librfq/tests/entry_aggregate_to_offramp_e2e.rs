#[cfg(feature = "proptest")]
mod test {
    use proptest::prelude::*;

    use bobcat_sdk::{storage::flush_guard_fresh, maths::strategies::*};

    use superposition_librfq::{
        entry_aggregate_to_offramp::recurse_aggregate_args, signable::SignableAggregateArgs,
        strategies::*, *,
    };

    proptest! {
        #[test]
        fn test_erik_alex_e2e(
            erik_addr in strat_addr_not_empty(),
            alex_addr in strat_addr_not_empty(),
            erik_ms_ts in strat_u8_6_not_empty(),
            alex_ms_ts in strat_u8_6_not_empty(),
            expiry in strat_u8_6_not_empty(),
            outcome in any::<Outcome>(),
            alex_amt in strat_u64_not_empty(),
            erik_amt in strat_u64_not_empty(),
            asset in any::<Asset>(),
            price_target in any::<u64>(),
            timestamplet_val in any::<u64>()
        ) {
            flush_guard_fresh(|| {
                storage::onramped_bal::set(&asset.into(), &erik_addr.into(), &erik_amt.into());
                storage::onramped_bal::set(&asset.into(), &alex_addr.into(), &alex_amt.into());
                let erik = BundleTaker {
                    args: BundleTakerArgs {
                        src: OnrampingSource::OnchainPot(OnchainPotArgs {
                            acc_addr: erik_addr,
                            amt: erik_amt,
                            asset,
                        }),
                        expiry,
                        ms_ts: erik_ms_ts,
                        is_up: true,
                        price_target,
                        outcome,
                    },
                    sig: [0u8; 64],
                };
                let alex = BundleMaker {
                    args: BundleMakerArgs {
                        taker: erik,
                        ms_ts: alex_ms_ts,
                        acc: alex_addr,
                        amt: alex_amt,
                    },
                    sig: [0u8; 64],
                };
                let agg = AggregateArgs {
                    maker: alex,
                    timestamplet: Timestamplet {
                        timestamplet_time: expiry,
                        timestamplet_outcome: outcome,
                        timestamplet_val,
                    },
                };
                let signable = SignableAggregateArgs::from(&agg);
                assert_eq!((erik_amt, erik_addr, asset.addr()), unsafe {
                    recurse_aggregate_args(&Imm::default(), &agg, &signable, &[0u8; 64]).unwrap()
                });
            })
        }
    }
}
