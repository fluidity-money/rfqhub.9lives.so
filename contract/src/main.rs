#![cfg_attr(target_arch = "wasm32", no_main)]
#![no_std]

#[cfg(target_arch = "wasm32")]
use bobcat_sdk::{bobcat_trace, entry::read_args_vec, storage::flush_guard};

#[cfg(feature = "arbos")]
use bobcat_sdk::entry::chain_id;

#[cfg(not(any(feature = "arbos", feature = "testing")))]
use bobcat_sdk::maths::U;

#[cfg(target_arch = "wasm32")]
use borsh::de::BorshDeserialize;

#[cfg(target_arch = "wasm32")]
use superposition_librfq::{Args, Imm, entry};

#[cfg(target_arch = "wasm32")]
mod host_addrs;

#[cfg(target_arch = "wasm32")]
macro_rules! env_word {
    ($name:ident, $input:literal) => {
        #[cfg(not(any(feature = "arbos", feature = "testing")))]
        const $name: U = U::const_from_hex(env!($input).as_bytes()).unwrap();
    };
}

#[cfg(target_arch = "wasm32")]
env_word!(SERVER_PUBLIC_KEY, "SPN_SERVER_PUBLIC_KEY");

#[unsafe(no_mangle)]
#[bobcat_trace]
#[cfg(target_arch = "wasm32")]
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
