#![cfg_attr(not(feature = "std"), no_std)]

use bobcat_sdk::{alloc::bobcat_allocator};

bobcat_allocator!();

use borsh::{BorshDeserialize, BorshSerialize};

pub mod immutables;

pub mod entry_onramping;
pub mod entry_submit;

pub mod call_accounts;
pub mod call_eip20;

use entry_onramping::entry_onramping;

pub use immutables::Imm;

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct FillerArgs {
    /// The amount of fUSDC that should be sent to the winner if they win.
    pub fusdc_on_win: u64,
    /// The sender of this blob. This is the account address. It verifies the signature
    /// and needs to have its assets parked already in the contract.
    pub acc_sender: [u8; 20],
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct PlayerArgs {}

pub type Sig = [u8; 64];

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub struct SubmitArgs {
    player_args: PlayerArgs,
    player_sig: Sig,
    filler_args: FillerArgs,
    filler_sig: Sig,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq, Debug)]
pub enum Args {
    Version,
    /// Dumps the source code of the contract using the bobcat-create harness.
    Source,
    /// When a user is onramping, they have a pot of assets that we add into here.
    Onramping {
        owner_passport: [u8; 20],
        amt: u64,
    },
}

pub fn entry(imm: &Imm, op: Args) -> usize {
    match op {
        Args::Onramping {
            owner_passport,
            amt,
        } => entry_onramping(&imm, owner_passport, amt),
        _ => panic!("bad argument"),
    }
}
