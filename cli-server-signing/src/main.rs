// cli-server-signing: Take the Args derivative structure and produce a
// signature that can be passed to sender to generate calldata with.

use std::{
    env::{args, var},
    io::stdin,
};

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use bobcat_sdk::precompiles::superposition::ed25519_sign_post;

use superposition_librfq::{
    signable::{Hash, SignableAggregateArgs, SignableOfframpArgs},
    AggregateArgs, OfframpArgs,
};

use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, SerdeDeserialize, SerdeSerialize)]
pub enum ServerSigningArgs {
    Offramp(OfframpArgs),
    AggregateToOfframp(AggregateArgs),
}

fn main() {
    let key = SigningKey::from_bytes(
        &const_hex::decode_to_array::<_, 32>(
            var("SPN_ACCOUNTS_PRIVATE_KEY")
                .unwrap()
                .trim_start_matches("0x"),
        )
        .unwrap(),
    );
    let args: ServerSigningArgs = if let Some(x) = args().nth(1) {
        serde_json::from_str::<_>(&x)
    } else {
        serde_json::from_reader::<_, _>(stdin())
    }
    .unwrap();
    println!(
        "{}",
        const_hex::encode(match args {
            ServerSigningArgs::Offramp(args) => ed25519_sign_post(key, SignableOfframpArgs::from(&args).hash),
            ServerSigningArgs::AggregateToOfframp(args) => {
                let Hash::JustInTime(hash) = SignableAggregateArgs::from(&args).hash else {
                    unreachable!()
                };
                ed25519_sign_post(key, hash)
            }
        })
    )
}
