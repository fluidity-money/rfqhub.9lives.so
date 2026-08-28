#![cfg_attr(target_arch = "wasm32", no_main)]
#![no_std]

use bobcat_sdk::{entry::read_args_vec, maths::U, storage::flush_guard};

#[cfg(feature = "arbos")]
use bobcat_sdk::entry::chain_id;

use borsh::de::BorshDeserialize;

use superposition_librfq::{Args, Imm, entry};

mod host_addrs;

macro_rules! env_word {
    ($name:ident, $input:literal) => {
        #[cfg(any(feature = "testing", not(target_arch = "wasm32")))]
        const $name: U = host_addrs::$name;
        #[cfg(not(any(feature = "testing", not(target_arch = "wasm32"))))]
        const $name: U = U::const_from_hex(env!($input).as_bytes()).unwrap();
    };
}

env_word!(SERVER_PUBLIC_KEY, "SPN_SERVER_PUBLIC_KEY");

#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_entrypoint(len: usize) -> usize {
    let args = read_args_vec(len);
    #[cfg(feature = "arbos")]
    let imm = Imm {
        should_server_sig_validate: false,
        should_user_sig_validate: false,
        server_key: SERVER_PUBLIC_KEY,
    };
    #[cfg(not(feature = "arbos"))]
    let imm = Imm {
        should_server_sig_validate: true,
        should_user_sig_validate: true,
        server_key: SERVER_PUBLIC_KEY,
    };
    #[cfg(feature = "arbos")]
    assert_eq!(31337, chain_id(), "chain id not arbos-forge");
    flush_guard(|| entry(&imm, Args::deserialize(&mut args.as_slice()).unwrap()))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
