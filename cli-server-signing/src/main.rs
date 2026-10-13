// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

// cli-server-signing: Take the Args derivative structure and produce a
// signature that can be passed to sender to generate calldata with.

use std::{
    env::{args, var},
    io::stdin,
};

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use bobcat_sdk::precompiles::superposition::ed25519_sign_post;

use superposition_librfq::{
    AggregateArgs, OfframpArgs,
    signable::{Hash, SignableAggregateArgs, SignableOfframpArgs},
};

use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, SerdeDeserialize, SerdeSerialize)]
pub enum ServerSigningArgs {
    Offramp(OfframpArgs),
    Aggregate(AggregateArgs),
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
            ServerSigningArgs::Offramp(args) =>
                ed25519_sign_post(key, SignableOfframpArgs::from(&args).hash),
            ServerSigningArgs::Aggregate(args) => {
                let Hash::JustInTime(hash) = SignableAggregateArgs::from(&args).hash else {
                    unreachable!()
                };
                ed25519_sign_post(key, hash)
            }
        })
    )
}
