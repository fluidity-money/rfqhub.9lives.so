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

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
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
pub struct OnchainPotArgs {
    pub acc_addr: [u8; 20],
    pub amt: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum OnrampingSource {
    /// Take the liquidity from the pot that the user already sent to the contract.
    OnchainPot(OnchainPotArgs),
    /// Take the liquidity from the result that we have here.
    AggregatedResult(Box<AggregateEnum>),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct BundleTakerArgs {
    /// Onramping source that we're using for loading the user in.
    pub src: OnrampingSource,
    /// The timestamp for when this RFQ market has expired.
    pub expiry: [u8; 6],
    /// The nonce that we use here. A millisecond timestamp.
    pub ms_ts: [u8; 6],
    /// The desired asset that the user is denominating this trade for.
    pub asset: Asset,
    /// Whether the taker wants the asset to go up or down.
    pub is_up: bool,
    /// The target of the price, which should be exceeded or lower than:
    pub price_target: u64,
    /// The outcome that's being speculated on here.
    pub outcome: Outcome,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum BundleTakerEnum {
    Args(BundleTakerArgs),
    Hash(U),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct BundleMakerArgs {
    /// The compensation for when this market is correct.
    pub taker: BundleTakerEnum,
    /// The nonce that we use here.
    pub ms_ts: [u8; 6],
    /// The account of the maker.
    pub acc: [u8; 20],
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct OnrampingFromBalArgs {
    /// The asset being provided for the onramping as this
    /// collateral. The "key" of the asset that's being used here.
    pub asset: Asset,
    /// The amount of collateral being provided for the
    pub amt: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct OfframpArgs {
    pub amt: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct Timestamplet {
    /// The timestamp for the value of this asset at the time given.
    pub timestamplet_time: [u8; 6],
    /// The outcome in use at the timestamplet.
    pub timestamplet_outcome: Outcome,
    /// The value of the asset at the time given.
    pub timestamplet_val: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct AggregateArgs {
    /// The maker bundle that we need to roll up here.
    pub maker: BundleMakerArgs,
    /// "Timestamplet" (signature from us that a timestamp for an asset
    /// is valid) for the time given.
    pub timestamplet: Timestamplet,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct Aggregate {
    pub args: AggregateArgs,
    pub server_sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum AggregateEnum {
    Args(Aggregate),
    Hash(U),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum Args {
    Version,
    /// Dumps the source code of the contract using the bobcat-create harness.
    Source,
    /// Get the available to spend onramping balance from the user:
    Balance {
        addr: [u8; 20],
    },
    /// When a user is onramping here, we're adding funds from their
    /// account to the pot that they can spend.
    OnrampingFromBal {
        args: OnrampingFromBalArgs,
        sig: Sig,
    },
    /// Simply offramp the amount in the pot that hasn't been spent.
    /// Requires a blessing from the server to make the offramping happen.
    Offramp {
        args: OfframpArgs,
        server_sig: Sig,
    },
    /// Aggregate a pair of bundles together, with a signature from the
    /// Superposition aggregator. Moves the end result of the pot to the
    /// takers and makers that were involved, and hashes the input data
    /// so that later we can reuse the input if we want without needing to
    /// supply the entire calldata. It will revert any users of the entrypoint
    /// here.
    AggregateToOfframp {
        args: AggregateArgs,
        /// For the aggregation here, we need to use the server signature to prevent abuse.
        server_sig: Sig,
    },
}

pub fn entry(imm: &Imm, op: Args) -> usize {
    match op {
        Args::Version => entry_version(),
        Args::Source => entry_source(),
        Args::Balance { addr } => entry_balance(addr),
        Args::OnrampingFromBal { args, sig } => entry_onramping_from_bal(args, sig),
        Args::Offramp { args, server_sig } => entry_offramp(&imm, args, server_sig),
        Args::AggregateToOfframp { args, server_sig } => {
            entry_aggregate_to_offramp(imm, args, server_sig)
        }
    }
}
