use crate::*;

use alloc::{boxed::Box, vec::Vec};
use core::{iter, slice};
use proptest::{arbitrary::StrategyFor, collection::vec, prelude::*};

const RECURSIVE_DEPTH: u32 = 4;
const RECURSIVE_MAX_SIZE: u32 = 64;
const RECURSIVE_ITEMS_PER_COLLECTION: u32 = 4;
const INVERTED_MAX_SUBMISSIONS: usize = 8;
const INVERTED_MAX_AMOUNT: u64 = 1_000_000_000_000;
const MAX_U48: u64 = (1_u64 << 48) - 1;

/// Direction recorded by the off-chain model for one RFQ settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceDirection {
    Up,
    Down,
}

impl PriceDirection {
    pub fn is_up(self) -> bool {
        matches!(self, Self::Up)
    }
}

/// The model's prediction and its expected taker balance after settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedPriceMove {
    pub direction: PriceDirection,
    pub expected_return: u64,
}

/// A chronological, non-recursive representation of one aggregate node.
///
/// `submitted_at` is trace-only metadata and is intentionally discarded by
/// `InvertedAggregate::to_aggregate_enum`, since the contract ABI has no such
/// field.
#[derive(Clone, Debug, PartialEq)]
pub struct InvertedSubmission {
    pub submitted_at: [u8; 6],
    pub expiry: [u8; 6],
    pub taker_ms_ts: [u8; 6],
    pub maker_ms_ts: [u8; 6],
    pub price_move: ExpectedPriceMove,
    pub price_target: u64,
    pub outcome: Outcome,
    pub maker_amt: u64,
    pub maker_acc: [u8; 20],
    pub taker_sig: Sig,
    pub maker_sig: Sig,
    pub timestamplet_val: u64,
    pub server_sig: Sig,
}

/// An "inside-out" aggregate: the funding leaf followed by settlements in
/// execution order. This is easier to inspect and model than the recursive ABI
/// representation in `lib.rs`.
#[derive(Clone, Debug, PartialEq)]
pub struct InvertedAggregate {
    pub initial_taker: OnchainPotArgs,
    pub first: InvertedSubmission,
    pub rest: Vec<InvertedSubmission>,
}

impl InvertedAggregate {
    pub fn submissions(
        &self,
    ) -> core::iter::Chain<iter::Once<&InvertedSubmission>, slice::Iter<'_, InvertedSubmission>>
    {
        iter::once(&self.first).chain(self.rest.iter())
    }

    /// Convert the chronological model back into the recursive contract ABI.
    pub fn to_aggregate_enum(&self) -> AggregateEnum {
        let mut source = OnrampingSource::OnchainPot(self.initial_taker.clone());
        let mut aggregate = None;

        for submission in self.submissions() {
            let next = AggregateEnum::Args(Aggregate {
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
            });
            source = OnrampingSource::AggregatedResult(Box::new(next.clone()));
            aggregate = Some(next);
        }

        // The type always contains `first`, so the loop cannot be empty.
        aggregate.expect("an inverted aggregate always has a first submission")
    }
}

#[derive(Clone, Debug)]
struct InvertedSubmissionSeed {
    direction: PriceDirection,
    price_target: u64,
    outcome: Outcome,
    maker_amt: u64,
    maker_acc: [u8; 20],
    taker_sig: Sig,
    maker_sig: Sig,
    timestamplet_val: u64,
    server_sig: Sig,
}

fn u48(value: u64) -> [u8; 6] {
    let bytes = value.to_be_bytes();
    bytes[2..].try_into().expect("six-byte slice")
}

fn inverted_submission_seed() -> impl Strategy<Value = InvertedSubmissionSeed> {
    (
        any::<bool>(),
        any::<u64>(),
        any::<Outcome>(),
        1..=INVERTED_MAX_AMOUNT,
        any::<[u8; 20]>(),
        sig(),
        sig(),
        any::<u64>(),
        sig(),
    )
        .prop_map(
            |(
                is_up,
                price_target,
                outcome,
                maker_amt,
                maker_acc,
                taker_sig,
                maker_sig,
                timestamplet_val,
                server_sig,
            )| InvertedSubmissionSeed {
                direction: if is_up {
                    PriceDirection::Up
                } else {
                    PriceDirection::Down
                },
                price_target,
                outcome,
                maker_amt,
                maker_acc,
                taker_sig,
                maker_sig,
                timestamplet_val,
                server_sig,
            },
        )
}

/// Generate only valid chronological model traces.
///
/// For every node: `taker_ms_ts < maker_ms_ts <= submitted_at < expiry`, and
/// the next node begins after the previous expiry. Amounts are bounded so the
/// generated expected-return arithmetic cannot overflow.
pub fn inverted_aggregate() -> BoxedStrategy<InvertedAggregate> {
    (
        any::<[u8; 20]>(),
        1..=INVERTED_MAX_AMOUNT,
        any::<Asset>(),
        0..=(MAX_U48 - (INVERTED_MAX_SUBMISSIONS as u64 * 5)),
        vec(inverted_submission_seed(), 1..=INVERTED_MAX_SUBMISSIONS),
    )
        .prop_map(|(acc_addr, amt, asset, base_time, seeds)| {
            let initial_taker = OnchainPotArgs {
                acc_addr,
                amt,
                asset,
            };
            let mut expected_return = amt;
            let mut submissions = Vec::with_capacity(seeds.len());

            for (index, seed) in seeds.into_iter().enumerate() {
                let start = base_time + index as u64 * 5;
                let did_taker_win = match seed.direction {
                    PriceDirection::Up => seed.timestamplet_val > seed.price_target,
                    PriceDirection::Down => seed.timestamplet_val <= seed.price_target,
                };
                expected_return = if did_taker_win {
                    expected_return
                        .checked_add(seed.maker_amt)
                        .expect("generated amounts are bounded")
                } else {
                    0
                };
                submissions.push(InvertedSubmission {
                    taker_ms_ts: u48(start),
                    maker_ms_ts: u48(start + 1),
                    submitted_at: u48(start + 2),
                    expiry: u48(start + 3),
                    price_move: ExpectedPriceMove {
                        direction: seed.direction,
                        expected_return,
                    },
                    price_target: seed.price_target,
                    outcome: seed.outcome,
                    maker_amt: seed.maker_amt,
                    maker_acc: seed.maker_acc,
                    taker_sig: seed.taker_sig,
                    maker_sig: seed.maker_sig,
                    timestamplet_val: seed.timestamplet_val,
                    server_sig: seed.server_sig,
                });
            }

            let first = submissions.remove(0);
            InvertedAggregate {
                initial_taker,
                first,
                rest: submissions,
            }
        })
        .boxed()
}

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
