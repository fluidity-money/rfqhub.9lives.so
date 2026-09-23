#![cfg_attr(target_arch = "wasm32", no_main)]
#![no_std]

use bobcat_sdk::{bobcat_trace, entry::read_args_vec, storage::flush_guard};

#[cfg(feature = "arbos")]
use bobcat_sdk::entry::chain_id;

#[cfg(not(feature = "arbos"))]
use bobcat_sdk::maths::U;

use borsh::de::BorshDeserialize;

use superposition_librfq::{Args, Imm, entry};

mod host_addrs;

macro_rules! env_word {
    ($name:ident, $input:literal) => {
        #[cfg(any(feature = "testing", not(target_arch = "wasm32")))]
        const $name: U = host_addrs::$name;
    };
}

env_word!(SERVER_PUBLIC_KEY, "SPN_SERVER_PUBLIC_KEY");

#[unsafe(no_mangle)]
#[bobcat_trace]
pub unsafe extern "C" fn user_entrypoint(len: usize) -> usize {
    let args = read_args_vec(len);
    #[cfg(feature = "arbos")]
    let imm = Imm::default();
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
