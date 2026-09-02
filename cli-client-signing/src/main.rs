// cli-client-signing: Take some arguments and generate the client side
// signature for a toplevel part of the Args that we send to the on-chain
// structure.

use std::{
    env::{args, var},
    io::stdin,
};

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use bobcat_sdk::precompiles::superposition::ed25519_sign_post;

use superposition_librfq::{
    BundleMakerArgs, BundleTakerArgs,
    signable::{Hash, SignableBundleMakerArgs, SignableBundleTakerArgs},
};

use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, SerdeDeserialize, SerdeSerialize)]
pub enum ClientSigningArgs {
    BundleTaker(BundleTakerArgs),
    BundleMaker(BundleMakerArgs),
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
    let args: ClientSigningArgs = if let Some(x) = args().nth(1) {
        serde_json::from_str::<_>(&x)
    } else {
        serde_json::from_reader::<_, _>(stdin())
    }
    .unwrap();
    println!(
        "{}",
        const_hex::encode(match args {
            ClientSigningArgs::BundleTaker(args) => {
                let Hash::JustInTime(hash) = SignableBundleTakerArgs::from(&args).hash else {
                    panic!("non just in time bundle taker not supported");
                };
                ed25519_sign_post(key, hash)
            }
            ClientSigningArgs::BundleMaker(args) => {
                let Hash::JustInTime(hash) = SignableBundleMakerArgs::from(&args).hash else {
                    panic!("non just in time bundle maker not supported");
                };
                ed25519_sign_post(key, hash)
            }
        })
    )
}
