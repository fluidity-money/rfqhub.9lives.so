// cli-signing: Take the Args derivative structure, but ignoring any
// existing signatures that are at the surface level, overriding what's
// there using the key we have here.

use std::env::{args, var};

use bobcat_sdk::precompiles::superposition::ed25519_sign_post;

use superposition_librfq::{
    Aggregate, Args, AggregateEnum,
    signable::{Hash, SignableAggregateArgs, SignableOfframpArgs},
};

use ed25519_dalek::SigningKey;

fn main() {
    let key = SigningKey::from_bytes(
        &const_hex::decode_to_array::<_, 32>(
            var("SPN_SUPERPOSITION_KEY")
                .unwrap()
                .trim_start_matches("0x"),
        )
        .unwrap(),
    );
    let arg1 = args().nth(1).unwrap();
    let args = serde_json::from_str::<Args>(&arg1).unwrap();
    println!(
        "{}",
        const_hex::encode(&match args {
            Args::Offramp { args, .. } => {
                ed25519_sign_post(key, SignableOfframpArgs::from(&args).hash)
            }
            Args::AggregateToOfframp {
                args: AggregateEnum::Args(agg),
                ..
            } => {
                let Aggregate { args, .. } = agg.as_ref();
                let Hash::JustInTime(hash) = SignableAggregateArgs::from(args).hash else {
                    panic!("non just in time hash tried to be signed");
                };
                ed25519_sign_post(key, hash)
            }
            Args::Version
            | Args::Source
            | Args::Balance { .. }
            | Args::OnrampingFromBal { .. }
            | Args::AggregateToOfframp { .. } => {
                panic!("unsupported hash signing")
            }
        })
    )
}
