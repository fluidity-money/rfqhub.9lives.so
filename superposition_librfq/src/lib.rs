#![cfg_attr(not(feature = "std"), no_std)]

use bobcat_sdk::{alloc::bobcat_allocator, cd::const_keccak256, maths::U};

bobcat_allocator!();

use borsh::{BorshDeserialize, BorshSerialize};

pub mod immutables;

pub mod entry_aggregate_to_offramp;
pub mod entry_balance;
pub mod entry_offramp;
pub mod entry_onramping_from_bal;
pub mod entry_source;
pub mod entry_version;

pub mod signable;

pub mod outcome;

pub mod call_aave_v3;
pub mod call_accounts;
pub mod call_eip20;

pub mod storage;

use entry_aggregate_to_offramp::entry_aggregate_to_offramp;
use entry_balance::entry_balance;
use entry_offramp::entry_offramp;
use entry_onramping_from_bal::entry_onramping_from_bal;
use entry_source::entry_source;
use entry_version::entry_version;

pub mod error;

pub use immutables::Imm;

pub use superposition_assets::Asset;

use outcome::Outcome;

use alloc::boxed::Box;

pub type Sig = [u8; 64];

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct FillerArgs {
    /// The amount of fUSDC that should be sent to the winner if they win.
    pub fusdc_on_win: u64,
    /// The sender of this blob. This is the account address. It verifies the signature
    /// and needs to have its assets parked already in the contract.
    pub acc_sender: [u8; 20],
}

/// Onramping topic that's emitted on the underlying balance changing
/// due to onrampind and offramping.
pub const TOPIC_BALANCE_CHANGED: U = const_keccak256(b"BalanceChanged(address,address,uint256)");

/// Global balance for the asset given has changed.
pub const TOPIC_GLOBAL_BALANCE_CHANGED: U =
    const_keccak256(b"GlobalBalanceChanged(address,uint256)");

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct OnchainPotArgs {
    pub acc_addr: [u8; 20],
    pub amt: u64,
    pub asset: Asset,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum OnrampingSource {
    /// Take the liquidity from the pot that the user already sent to the contract.
    OnchainPot(OnchainPotArgs),
    /// Take the liquidity from the result that we have here.
    AggregatedResult(Box<AggregateEnum>),
    /// Combination of the aggregated result from before and an onramping amount. TODO.
    OnchainPotAggregatedResult(OnchainPotArgs, Box<AggregateEnum>),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct BundleTakerArgs {
    /// Onramping source that we're using for loading the user in.
    pub src: OnrampingSource,
    /// The timestamp for when this RFQ market has expired.
    pub expiry: [u8; 6],
    /// The nonce that we use here. A millisecond timestamp.
    pub ms_ts: [u8; 6],
    /// Whether the taker wants the asset to go up or down.
    pub is_up: bool,
    /// The target of the price, which should be exceeded or lower than:
    pub price_target: u64,
    /// The outcome that's being speculated on here.
    pub outcome: Outcome,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct BundleTaker {
    pub args: BundleTakerArgs,
    pub sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct BundleMakerArgs {
    /// The compensation for if the taker is correct:
    pub amt: u64,
    /// The taker this is a part of:
    pub taker: BundleTaker,
    /// The nonce that we use here.
    pub ms_ts: [u8; 6],
    /// The account of the maker.
    pub acc: [u8; 20],
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct BundleMaker {
    pub args: BundleMakerArgs,
    /// User signature for this maker bundle.
    pub sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct OfframpArgs {
    pub amt: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct Timestamplet {
    /// The timestamp for the value of this asset at the time given.
    pub timestamplet_time: [u8; 6],
    /// The outcome in use at the timestamplet.
    pub timestamplet_outcome: Outcome,
    /// The value of the asset at the time given.
    pub timestamplet_val: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct AggregateArgs {
    /// The maker bundle that we need to roll up here.
    pub maker: BundleMaker,
    /// "Timestamplet" (signature from us that a timestamp for an asset
    /// is valid) for the time given.
    pub timestamplet: Timestamplet,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct Aggregate {
    pub args: AggregateArgs,
    pub server_sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum AggregateEnum {
    Args(Aggregate),
    Hash(U),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum Args {
    Version,
    /// Dumps the source code of the contract using the bobcat-create harness.
    Source,
    /// Get the available to spend onramping balance from the user:
    Balance {
        asset: [u8; 20],
        addr: [u8; 20],
    },
    /// When a user is onramping here, we're adding funds from their
    /// account to the pot that they can spend.
    OnrampingFromBal {
        /// The asset being provided for the onramping as this
        /// collateral. The "key" of the asset that's being used here.
        asset: Asset,
        /// The amount of collateral being provided for the
        amt: u64,
    },
    /// Simply offramp the amount in the pot that hasn't been spent.
    /// Requires a blessing from the server to make the offramping happen.
    Offramp {
        args: OfframpArgs,
        server_sig: Sig,
    },
    /// Aggregate a pair of bundles together, with a signature from the
    /// Superposition aggregator. Offramps the taker that was involved
    /// in the submission of the data.
    AggregateToOfframp {
        args: AggregateEnum,
        is_taker: bool,
    },
}

#[cfg(feature = "proptest")]
pub mod proptest_strategies {
    use super::*;
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

    fn aggregate_strategy_with_source(
        src: BoxedStrategy<OnrampingSource>,
    ) -> BoxedStrategy<Aggregate> {
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
}

pub fn entry(imm: &Imm, op: Args) -> usize {
    match op {
        Args::Version => entry_version(),
        Args::Source => entry_source(),
        Args::Balance { asset, addr } => entry_balance(asset, addr),
        Args::OnrampingFromBal { asset, amt } => entry_onramping_from_bal(asset, amt),
        Args::Offramp { args, server_sig } => entry_offramp(&imm, args, server_sig),
        Args::AggregateToOfframp { args, is_taker } => {
            entry_aggregate_to_offramp(imm, args, is_taker)
        }
    }
}
