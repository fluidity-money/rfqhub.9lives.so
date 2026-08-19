///! Shadow structure that can be easily signed eagerly to the format
/// we use to reconstruct signers. Inner structures are used for the
/// value part of the signing, with the Signable structures being used
/// to recurse through to retrieve hashes. Hashed structures work like this:
/// [DISCRIMINT | CONTENT HASHED WITH INNER | CONTENT BEFORE THIS ITEM* ]
/// The signatures are compressed using keccak256 when they are
/// aggregated, but when they are signed, they are used in their original
/// [u8; 64] form. If they are used in the context where a signer tries to
/// sign something that is on-chain, this is an error. The content is
/// signed as a sha512. End users signing the eventual blob that must
/// be checked with the accounts engine need to prefix their blob with
/// "Accounts statement", and the address of the account doing the
/// verifying, then the content, in a hex form without a 0x prefix.
use borsh::BorshSerialize;

use crate::{
    Aggregate, AggregateArgs, AggregateEnum, BundleMaker, BundleMakerArgs, BundleTaker,
    BundleTakerArgs, OnrampingSource, Outcome,
};

use array_concat::concat_arrays;

use bobcat_sdk::{maths::U, precompiles::superposition::sha512, storage::keccak256};

use alloc::boxed::Box;

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub enum Hash {
    Onchain(U),
    JustInTime([u8; 64]),
}

impl From<Hash> for [u8; 32] {
    fn from(x: Hash) -> Self {
        match x {
            Hash::Onchain(U(u)) => u,
            Hash::JustInTime(x) => *keccak256(&x),
        }
    }
}

impl From<Hash> for U {
    fn from(x: Hash) -> Self {
        U(x.into())
    }
}

#[repr(u8)]
#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub enum PreDiscriminant {
    AggregateEnum,
    BundleMakerArgs,
    BundleTakerArgs,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableBundleTakerArgs {
    pub aggregated_res: Option<Box<SignableAggregateArgs>>,
    pub hash: Hash,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct InnerBundleTakerArgs {
    pub expiry: [u8; 6],
    pub ms_ts: [u8; 6],
    pub is_up: bool,
    pub price_target: u64,
    pub outcome: Outcome,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct InnerBundleMaker {
    pub ms_ts: [u8; 6],
    pub acc: [u8; 20],
    pub amt: u64,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableBundleMakerArgs {
    pub taker_args: SignableBundleTakerArgs,
    pub hash: Hash,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableAggregateArgs {
    pub maker: SignableBundleMakerArgs,
    pub hash: Hash,
}

impl From<&AggregateArgs> for SignableAggregateArgs {
    fn from(
        AggregateArgs {
            maker: BundleMaker {
                args: maker_args, ..
            },
            timestamplet,
            ..
        }: &AggregateArgs,
    ) -> Self {
        let maker = SignableBundleMakerArgs::from(maker_args);
        let timestamplet = sha512(&borsh::to_vec(&timestamplet).unwrap());
        let pre: [u8; 1 + 32 + 64] = concat_arrays!(
            [PreDiscriminant::AggregateEnum as u8],
            maker.hash.clone().into(),
            timestamplet
        );
        Self {
            maker,
            hash: Hash::JustInTime(sha512(&pre)),
        }
    }
}

impl From<&BundleMakerArgs> for SignableBundleMakerArgs {
    fn from(
        BundleMakerArgs {
            taker: BundleTaker { args, .. },
            ms_ts,
            acc,
            amt,
        }: &BundleMakerArgs,
    ) -> Self {
        let taker_pre = SignableBundleTakerArgs::from(args);
        let content = sha512(
            &borsh::to_vec(&InnerBundleMaker {
                ms_ts: *ms_ts,
                acc: *acc,
                amt: *amt,
            })
            .unwrap(),
        );
        let pre: [u8; 1 + 64 + 32] = concat_arrays!(
            [PreDiscriminant::BundleMakerArgs as u8],
            content,
            taker_pre.hash.clone().into()
        );
        Self {
            taker_args: taker_pre,
            hash: Hash::JustInTime(sha512(&pre)),
        }
    }
}

impl From<&BundleTakerArgs> for SignableBundleTakerArgs {
    fn from(
        BundleTakerArgs {
            src,
            expiry,
            ms_ts,
            is_up,
            price_target,
            outcome,
        }: &BundleTakerArgs,
    ) -> Self {
        let (onramping_pre, onramping_hash) = match src {
            OnrampingSource::OnchainPot(args) => (
                None,
                Hash::JustInTime(sha512(&borsh::to_vec(&args).unwrap())),
            ),
            OnrampingSource::AggregatedResult(args) => match *args.clone() {
                AggregateEnum::Args(Aggregate { args, .. }) => {
                    let pre = SignableAggregateArgs::from(&args);
                    let hash = pre.hash.clone();
                    (Some(Box::new(pre)), hash)
                }
                AggregateEnum::Hash(u) => (None, Hash::Onchain(u)),
            },
        };
        let content = sha512(
            &borsh::to_vec(&InnerBundleTakerArgs {
                expiry: *expiry,
                ms_ts: *ms_ts,
                is_up: *is_up,
                price_target: *price_target,
                outcome: *outcome,
            })
            .unwrap(),
        );
        let pre: [u8; 1 + 64 + 32] = concat_arrays!(
            [PreDiscriminant::BundleTakerArgs as u8],
            content,
            onramping_hash.into()
        );
        Self {
            aggregated_res: onramping_pre,
            hash: Hash::JustInTime(sha512(&pre)),
        }
    }
}
