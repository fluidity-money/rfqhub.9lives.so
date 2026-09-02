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
pub mod version;

pub mod signable;

pub mod outcome;

pub mod call_aave_v3;
pub mod call_accounts;

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
pub mod std_eip20;
#[cfg(any(not(feature = "std"), target_arch = "wasm32"))]
pub mod wasm_eip20;

pub mod call_eip20;

pub mod storage;

pub use version::Version;

pub mod events;

use entry_aggregate_to_offramp::entry_aggregate_to_offramp;
use entry_balance::entry_balance;
use entry_offramp::entry_offramp;
use entry_onramping_from_bal::entry_onramping_from_bal;
use entry_source::entry_source;
use entry_version::entry_version;

pub mod error;

pub use immutables::Imm;

pub use superposition_assets::Asset;

pub use outcome::Outcome;

use alloc::boxed::Box;

pub type Sig = [u8; 64];

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;

#[cfg(feature = "serde")]
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

#[cfg(all(feature = "arbos", feature = "arbitrum"))]
compile_error!("arbos and arbitrum cannot both be enabled");

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct FillerArgs {
    /// The amount of fUSDC that should be sent to the winner if they win.
    pub fusdc_on_win: u64,
    /// The sender of this blob. This is the account address. It verifies the signature
    /// and needs to have its assets parked already in the contract.
    pub acc_sender: [u8; 20],
}

/// Onramping topic that's emitted on the underlying balance changing
/// due to onrampind and offramping.
pub const TOPIC_BALANCE_CHANGED: U =
    const_keccak256(b"BalanceChanged(address,address,uint64,unt256)");

/// Global balance for the asset given has changed.
pub const TOPIC_GLOBAL_BALANCE_CHANGED: U =
    const_keccak256(b"GlobalBalanceChanged(address,uint256,uint256)");

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct OnchainPotArgs {
    pub acc_addr: [u8; 20],
    pub amt: u64,
    pub asset: Asset,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct AggregatedResultArgs {
    pub from: Box<AggregateEnum>,
    /// Amount to take from the aggregated result from before. The rest
    /// should be committed to the Onboarding pot. The same user
    /// experience as before is that this can only be consumed once.
    pub amt_to_take: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub enum OnrampingSource {
    /// Take the liquidity from the pot that the user already sent to the contract.
    OnchainPot(OnchainPotArgs),
    /// Take the liquidity from the result that we have here.
    AggregatedResult(AggregatedResultArgs),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
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
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct BundleTaker {
    pub args: BundleTakerArgs,
    #[cfg_attr(feature = "serde", serde(with = "const_hex"))]
    pub sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct BundleMakerArgs {
    /// The compensation for if the taker is correct:
    pub src: OnrampingSource,
    /// The taker this is a part of:
    pub taker: BundleTaker,
    /// The nonce that we use here.
    pub ms_ts: [u8; 6],
    /// The account of the maker.
    pub acc: [u8; 20],
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct BundleMaker {
    pub args: BundleMakerArgs,
    /// User signature for this maker bundle.
    #[cfg_attr(feature = "serde", serde(with = "const_hex"))]
    pub sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct OfframpArgs {
    /// The asset being used here.
    pub asset: Asset,
    /// The amount being taken out of the game.
    pub amt: u64,
    /// The timestamp of the offramp request we have here.
    pub ms_ts: [u8; 6],
    /// The Accounts address in use for the amount here.
    pub acc_addr: [u8; 20],
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
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
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct AggregateArgs {
    /// The maker bundle that we need to roll up here.
    pub maker: BundleMaker,
    /// "Timestamplet" (signature from us that a timestamp for an asset
    /// is valid) for the time given.
    pub timestamplet: Timestamplet,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub struct Aggregate {
    pub args: AggregateArgs,
    #[cfg_attr(feature = "serde", serde(with = "const_hex"))]
    pub server_sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
pub enum AggregateEnum {
    Args(Box<Aggregate>),
    Hash(U),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(SerdeDeserialize, SerdeSerialize))]
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
        #[cfg_attr(feature = "serde", serde(with = "const_hex"))]
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
pub mod strategies;

pub fn entry(imm: &Imm, op: Args) -> usize {
    match op {
        Args::Version => entry_version(),
        Args::Source => entry_source(),
        Args::Balance { asset, addr } => entry_balance(asset, addr),
        Args::OnrampingFromBal { asset, amt } => entry_onramping_from_bal(asset, amt),
        Args::Offramp { args, server_sig } => entry_offramp(imm, args, server_sig),
        Args::AggregateToOfframp { args, is_taker } => {
            entry_aggregate_to_offramp(imm, args, is_taker)
        }
    }
}
