use crate::{
    AggregateArgs, AggregateEnum, BundleMakerArgs, BundleTakerArgs, BundleTakerEnum,
    OnchainPotArgs, OnrampingSource, storage,
    Sig,
};

fn taker_addr_aggregated_res(r: &AggregateEnum) -> [u8; 20] {
    match r {
        AggregateEnum::Args(AggregateArgs {
            maker: BundleMakerArgs { taker, .. },
        }) => taker_addr(&taker),
        AggregateEnum::Hash(hash) => storage::hash_taker_addr::get(hash.into()).into(),
    }
}

/// Get the taker address from the bundle taker enum.
pub fn taker_addr(x: &BundleTakerEnum) -> [u8; 20] {
    match x {
        BundleTakerEnum::Args(BundleTakerArgs {
            src: OnrampingSource::OnchainPot(OnchainPotArgs { acc_addr, .. }),
            ..
        }) => *acc_addr,
        BundleTakerEnum::Args(BundleTakerArgs {
            src: OnrampingSource::AggregatedResult(r),
            ..
        }) => taker_addr_aggregated_res(r),
        BundleTakerEnum::Hash(hash) => storage::hash_taker_addr::get(hash.into()).into(),
    }
}

pub fn entry_aggregate_to_offramp(x: AggregateArgs, server_sig: Sig) -> usize {
    0
}
