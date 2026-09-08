use ed25519_dalek::SigningKey;

use bobcat_sdk::precompiles::superposition::{BcSha512, ed25519_sign_post};

use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1);
    let d: [u8; 64] = const_hex::decode(args.next().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let d = BcSha512(d);
    let k = SigningKey::from_bytes(
        &const_hex::decode(std::env::var("SPN_ACCOUNTS_PRIVATE_KEY").unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    );
    std::io::stdout()
        .write_all(&ed25519_sign_post(k, d))
        .unwrap();
}
