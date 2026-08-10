#![cfg_attr(not(feature = "std"), no_std)]

use bobcat_sdk::{alloc::bobcat_allocator, cd::const_keccak256, maths::U};

bobcat_allocator!();

use borsh::{BorshDeserialize, BorshSerialize};

pub mod immutables;

pub mod entry_balance;
pub mod entry_onramping;
pub mod entry_submit;

pub mod call_aave_v3;
pub mod call_accounts;
pub mod call_eip20;

pub mod storage;

use entry_balance::entry_balance;
use entry_onramping::entry_onramping;

pub use immutables::Imm;

pub use superposition_assets::Asset;

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
pub const TOPIC_BALANCE_CHANGED: U = const_keccak256(b"BalanceChanged(address,uint256)");

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum OnrampingSource {
    /// Take the liquidity from the pot that the user already sent to the contract.
    OnchainPot(u64),
    /// Take the liquidity from the result that we have here.
    AggregatedResult(Box<AggregateEnum>),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct BundleTakerArgs {
    /// Onramping source that we're using
    src: OnrampingSource,
    /// The timestamp for when this RFQ market has expired.
    expiry: u64,
    /// The nonce that we use here.
    ms_ts: u16,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum BundleTakerEnum {
    Args(BundleTakerArgs),
    Enum([u8; 32]),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct BundleMakerArgs {
    /// The compensation for when this market is correct.
    taker: BundleTaker,
    /// The nonce that we use here.
    ms_ts: u16,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct OnrampingFromBalArgs {
    /// The asset being provided for the onramping as this
    /// collateral. The "key" of the asset that's being used here.
    asset: Asset,
    /// The owner's Superposition Account server for verification.
    owner_acc: [u8; 20],
    /// The amount of collateral being provided for the
    amt: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum OnrampingFromBalEnum {
    Args(OnrampingFromBalArgs),
    Hash([u8; 32]),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct AggregateArgs {
    /// The maker bundle that we need to roll up here.
    pub maker: BundleMakerArgs,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum AggregateEnum {
    Args(AggregateArgs),
    Hash([u8; 32]),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct Aggregate {
    args: AggregateEnum,
    /// For the aggregation here, we need to use the server signature to prevent abuse.
    server_sig: Sig,
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
        args: OnrampingFromBalEnum,
        sig: Sig,
    },
    /// Aggregate a pair of bundles together, with a signature from the
    /// Superposition aggregator. Moves the end result of the pot to the
    /// takers and makers that were involved, and hashes the input data
    /// so that later we can reuse the input if we want without needing to
    /// supply the entire calldata. It will revert any users of the entrypoint
    /// here.
    AggregateToOfframp(Aggregate),
}

pub fn entry(imm: &Imm, op: Args) -> usize {
    match op {
        Args::Balance { addr } => entry_balance(addr),
        Args::Onramping {
            owner_passport,
            amt,
        } => entry_onramping(&imm, owner_passport, amt),
        _ => panic!("bad argument"),
    }
}
