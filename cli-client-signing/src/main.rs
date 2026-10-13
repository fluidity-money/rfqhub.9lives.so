// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

// cli-client-signing: Take some arguments and generate the client side
// signature for a toplevel part of the Args that we send to the on-chain
// structure.

use std::{
    env::{args, var},
    io::stdin,
};

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

use bobcat_sdk::maths::U;

use superposition_librfq::{
    BundleMakerArgs, BundleTakerArgs,
    signable::{SignableBundleMakerArgs, SignableBundleTakerArgs},
};

use superposition_libaccounts::sign_statement;

use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, SerdeDeserialize, SerdeSerialize)]
pub enum ClientSigningBundle {
    BundleTaker(BundleTakerArgs),
    BundleMaker(BundleMakerArgs),
}

#[derive(Debug, Clone, SerdeDeserialize, SerdeSerialize)]
pub struct ClientSigningArgs {
    pub acc_addr: [u8; 20],
    pub bundle: ClientSigningBundle,
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
    let ClientSigningArgs { acc_addr, bundle } = if let Some(x) = args().nth(1) {
        serde_json::from_str::<_>(&x)
    } else {
        serde_json::from_reader::<_, _>(stdin())
    }
    .unwrap();
    let (hash, ms_ts) = match bundle {
        ClientSigningBundle::BundleTaker(args) => {
            (SignableBundleTakerArgs::from(&args).hash, args.ms_ts)
        }
        ClientSigningBundle::BundleMaker(args) => {
            (SignableBundleMakerArgs::from(&args).hash, args.ms_ts)
        }
    };
    let hash: U = hash.into();
    let sig = sign_statement(key, hash.as_vec(), ms_ts, acc_addr);
    println!("{}", const_hex::encode(sig.0))
}
