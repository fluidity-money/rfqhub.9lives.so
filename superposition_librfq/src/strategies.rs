use crate::*;

use proptest::{arbitrary::StrategyFor, prelude::*};

const RECURSIVE_DEPTH: u32 = 4;
const RECURSIVE_MAX_SIZE: u32 = 64;
const RECURSIVE_ITEMS_PER_COLLECTION: u32 = 4;

fn sig() -> impl Strategy<Value = Sig> {
    any::<[u8; 64]>()
}

fn onramping_source_strategy() -> BoxedStrategy<OnrampingSource> {
    let leaf = any::<OnchainPotArgs>()
        .prop_map(OnrampingSource::OnchainPot)
        .boxed();

    leaf.prop_recursive(
        RECURSIVE_DEPTH,
        RECURSIVE_MAX_SIZE,
        RECURSIVE_ITEMS_PER_COLLECTION,
        |inner| {
            prop_oneof![
                aggregate_enum_strategy_with_source(inner.clone())
                    .prop_map(|args| { OnrampingSource::AggregatedResult(Box::new(args)) }),
                (
                    any::<OnchainPotArgs>(),
                    aggregate_enum_strategy_with_source(inner)
                )
                    .prop_map(|(pot, args)| {
                        OnrampingSource::OnchainPotAggregatedResult(pot, Box::new(args))
                    },),
            ]
        },
    )
    .boxed()
}

fn bundle_taker_args_strategy(
    src: BoxedStrategy<OnrampingSource>,
) -> BoxedStrategy<BundleTakerArgs> {
    (
        src,
        any::<[u8; 6]>(),
        any::<[u8; 6]>(),
        any::<bool>(),
        any::<u64>(),
        any::<Outcome>(),
    )
        .prop_map(
            |(src, expiry, ms_ts, is_up, price_target, outcome)| BundleTakerArgs {
                src,
                expiry,
                ms_ts,
                is_up,
                price_target,
                outcome,
            },
        )
        .boxed()
}

fn bundle_taker_strategy(src: BoxedStrategy<OnrampingSource>) -> BoxedStrategy<BundleTaker> {
    (bundle_taker_args_strategy(src), sig())
        .prop_map(|(args, sig)| BundleTaker { args, sig })
        .boxed()
}

fn bundle_maker_args_strategy(
    src: BoxedStrategy<OnrampingSource>,
) -> BoxedStrategy<BundleMakerArgs> {
    (
        any::<u64>(),
        bundle_taker_strategy(src),
        any::<[u8; 6]>(),
        any::<[u8; 20]>(),
    )
        .prop_map(|(amt, taker, ms_ts, acc)| BundleMakerArgs {
            amt,
            taker,
            ms_ts,
            acc,
        })
        .boxed()
}

fn bundle_maker_strategy(src: BoxedStrategy<OnrampingSource>) -> BoxedStrategy<BundleMaker> {
    (bundle_maker_args_strategy(src), sig())
        .prop_map(|(args, sig)| BundleMaker { args, sig })
        .boxed()
}

fn timestamplet_strategy() -> BoxedStrategy<Timestamplet> {
    (any::<[u8; 6]>(), any::<Outcome>(), any::<u64>())
        .prop_map(
            |(timestamplet_time, timestamplet_outcome, timestamplet_val)| Timestamplet {
                timestamplet_time,
                timestamplet_outcome,
                timestamplet_val,
            },
        )
        .boxed()
}

fn aggregate_args_strategy_with_source(
    src: BoxedStrategy<OnrampingSource>,
) -> BoxedStrategy<AggregateArgs> {
    (bundle_maker_strategy(src), timestamplet_strategy())
        .prop_map(|(maker, timestamplet)| AggregateArgs {
            maker,
            timestamplet,
        })
        .boxed()
}

fn aggregate_strategy_with_source(src: BoxedStrategy<OnrampingSource>) -> BoxedStrategy<Aggregate> {
    (aggregate_args_strategy_with_source(src), sig())
        .prop_map(|(args, server_sig)| Aggregate { args, server_sig })
        .boxed()
}

fn aggregate_enum_strategy_with_source(
    src: BoxedStrategy<OnrampingSource>,
) -> BoxedStrategy<AggregateEnum> {
    prop_oneof![
        aggregate_strategy_with_source(src).prop_map(AggregateEnum::Args),
        any::<U>().prop_map(AggregateEnum::Hash),
    ]
    .boxed()
}

impl proptest::arbitrary::Arbitrary for FillerArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<u64>(), any::<[u8; 20]>())
            .prop_map(|(fusdc_on_win, acc_sender)| FillerArgs {
                fusdc_on_win,
                acc_sender,
            })
            .boxed()
    }
}

impl proptest::arbitrary::Arbitrary for OnchainPotArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<[u8; 20]>(), any::<u64>(), any::<Asset>())
            .prop_map(|(acc_addr, amt, asset)| OnchainPotArgs {
                acc_addr,
                amt,
                asset,
            })
            .boxed()
    }
}

impl proptest::arbitrary::Arbitrary for OnrampingSource {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        onramping_source_strategy()
    }
}

impl proptest::arbitrary::Arbitrary for BundleTakerArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        bundle_taker_args_strategy(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for BundleTaker {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        bundle_taker_strategy(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for BundleMakerArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        bundle_maker_args_strategy(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for BundleMaker {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        bundle_maker_strategy(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for OfframpArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<u64>().prop_map(|amt| OfframpArgs { amt }).boxed()
    }
}

impl proptest::arbitrary::Arbitrary for Timestamplet {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        timestamplet_strategy()
    }
}

impl proptest::arbitrary::Arbitrary for AggregateArgs {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        aggregate_args_strategy_with_source(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for Aggregate {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        aggregate_strategy_with_source(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for AggregateEnum {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        aggregate_enum_strategy_with_source(any::<OnrampingSource>().boxed())
    }
}

impl proptest::arbitrary::Arbitrary for Args {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Args::Version),
            Just(Args::Source),
            (any::<[u8; 20]>(), any::<[u8; 20]>())
                .prop_map(|(asset, addr)| { Args::Balance { asset, addr } }),
            (any::<Asset>(), any::<u64>())
                .prop_map(|(asset, amt)| { Args::OnrampingFromBal { asset, amt } }),
            (any::<OfframpArgs>(), sig())
                .prop_map(|(args, server_sig)| Args::Offramp { args, server_sig }),
            (any::<AggregateEnum>(), any::<bool>())
                .prop_map(|(args, is_taker)| { Args::AggregateToOfframp { args, is_taker } }),
        ]
        .boxed()
    }
}

pub fn filler_args() -> StrategyFor<FillerArgs> {
    any::<FillerArgs>()
}

pub fn onchain_pot_args() -> StrategyFor<OnchainPotArgs> {
    any::<OnchainPotArgs>()
}

pub fn onramping_source() -> StrategyFor<OnrampingSource> {
    any::<OnrampingSource>()
}

pub fn bundle_taker_args() -> StrategyFor<BundleTakerArgs> {
    any::<BundleTakerArgs>()
}

pub fn bundle_taker() -> StrategyFor<BundleTaker> {
    any::<BundleTaker>()
}

pub fn bundle_maker_args() -> StrategyFor<BundleMakerArgs> {
    any::<BundleMakerArgs>()
}

pub fn bundle_maker() -> StrategyFor<BundleMaker> {
    any::<BundleMaker>()
}

pub fn offramp_args() -> StrategyFor<OfframpArgs> {
    any::<OfframpArgs>()
}

pub fn timestamplet() -> StrategyFor<Timestamplet> {
    any::<Timestamplet>()
}

pub fn aggregate_args() -> StrategyFor<AggregateArgs> {
    any::<AggregateArgs>()
}

pub fn aggregate() -> StrategyFor<Aggregate> {
    any::<Aggregate>()
}

pub fn aggregate_enum() -> StrategyFor<AggregateEnum> {
    any::<AggregateEnum>()
}

pub fn args() -> StrategyFor<Args> {
    any::<Args>()
}

pub fn strat_u8_6_not_empty() -> impl Strategy<Value = [u8; 6]> {
    any::<[u8; 6]>().prop_filter("[u8; 6] must be non-zero", |x| {
        x.iter().any(|&byte| byte != 0)
    })
}

pub fn strat_u64_not_empty() -> impl Strategy<Value = u64> {
    1..=u64::MAX
}
