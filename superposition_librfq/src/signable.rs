///! Shadow structure that can be easily signed eagerly to the format
/// we use to reconstruct signers. Inner structures are used for the
/// value part of the signing, with the Signable structures being used
/// to recurse through to retrieve hashes. Hashed structures work like this:
/// [DISCRIMINT | CONTENT HASHED WITH INNER | CONTENT BEFORE THIS ITEM* ]
use borsh::BorshSerialize;

use crate::{
    Aggregate, AggregateArgs, AggregateEnum, Asset, BundleMakerArgs, BundleTakerArgs,
    BundleTakerEnum, OnrampingSource, Outcome, Timestamplet,
};

use array_concat::concat_arrays;

use bobcat_sdk::{maths::U, storage::keccak256};

use alloc::boxed::Box;

pub type Hash = U;

#[repr(u8)]
#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub enum PreDiscriminant {
    AggregateEnum,
    BundleMakerArgs,
    BundleTakerArgs,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableBundleTakerArgs {
    pub aggregated_res: Option<Box<SignableAggregate>>,
    pub hash: Hash,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct InnerBundleTakerArgs {
    pub expiry: [u8; 6],
    pub ms_ts: [u8; 6],
    pub asset: Asset,
    pub is_up: bool,
    pub price_target: u64,
    pub outcome: Outcome,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct InnerBundleMaker {
    pub ms_ts: [u8; 6],
    pub acc: [u8; 20],
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableBundleMakerArgs {
    pub taker_args: Option<SignableBundleTakerArgs>,
    pub hash: Hash,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableTimestamplet {
    pub hash: Hash,
}

#[derive(BorshSerialize, Clone, PartialEq, Debug)]
pub struct SignableAggregate {
    pub maker: Option<SignableBundleMakerArgs>,
    pub timestamplet: Option<SignableTimestamplet>,
    pub hash: Hash,
}

impl From<AggregateEnum> for SignableAggregate {
    fn from(x: AggregateEnum) -> Self {
        match x {
            AggregateEnum::Args(Aggregate {
                args:
                    AggregateArgs {
                        maker,
                        timestamplet,
                    },
                ..
            }) => {
                let maker = SignableBundleMakerArgs::from(maker);
                let timestamplet = SignableTimestamplet::from(timestamplet);
                let pre: [u8; 1 + 32 * 2] = concat_arrays!(
                    [PreDiscriminant::AggregateEnum as u8],
                    *maker.hash,
                    *timestamplet.hash
                );
                Self {
                    maker: Some(maker),
                    timestamplet: Some(timestamplet),
                    hash: keccak256(&pre),
                }
            }
            AggregateEnum::Hash(h) => Self {
                maker: None,
                timestamplet: None,
                hash: h,
            },
        }
    }
}

impl From<Timestamplet> for SignableTimestamplet {
    fn from(x: Timestamplet) -> Self {
        Self {
            hash: keccak256(&borsh::to_vec(&x).unwrap()),
        }
    }
}

impl From<BundleMakerArgs> for SignableBundleMakerArgs {
    fn from(BundleMakerArgs { taker, ms_ts, acc }: BundleMakerArgs) -> Self {
        let (taker_pre, taker_hash) = match taker {
            BundleTakerEnum::Args(args) => {
                let taker = SignableBundleTakerArgs::from(args);
                let hash = taker.hash.clone();
                (Some(taker), hash)
            }
            BundleTakerEnum::Hash(h) => (None, h),
        };
        let content = keccak256(&borsh::to_vec(&InnerBundleMaker { ms_ts, acc }).unwrap());
        let pre: [u8; 1 + 32 + 32] = concat_arrays!(
            [PreDiscriminant::BundleMakerArgs as u8],
            *content,
            *taker_hash
        );
        Self {
            taker_args: taker_pre,
            hash: keccak256(&pre),
        }
    }
}

impl From<BundleTakerArgs> for SignableBundleTakerArgs {
    fn from(
        BundleTakerArgs {
            src,
            expiry,
            ms_ts,
            asset,
            is_up,
            price_target,
            outcome,
        }: BundleTakerArgs,
    ) -> Self {
        let (onramping_pre, onramping_hash) = match src {
            OnrampingSource::OnchainPot(args) => (None, keccak256(&borsh::to_vec(&args).unwrap())),
            OnrampingSource::AggregatedResult(res) => {
                let pre = SignableAggregate::from(*res);
                let hash = pre.hash.clone();
                (Some(Box::new(pre)), hash)
            }
        };
        let content = keccak256(
            &borsh::to_vec(&InnerBundleTakerArgs {
                expiry,
                ms_ts,
                asset,
                is_up,
                price_target,
                outcome,
            })
            .unwrap(),
        );
        let pre: [u8; 1 + 32 * 2] = concat_arrays!(
            [PreDiscriminant::BundleTakerArgs as u8],
            *content,
            *onramping_hash
        );
        Self {
            aggregated_res: onramping_pre,
            hash: keccak256(&pre),
        }
    }
}
