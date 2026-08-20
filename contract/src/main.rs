#![cfg_attr(not(feature = "testing"), no_main)]
#![no_std]

use bobcat_sdk::{entry::read_args_vec, maths::U, storage::flush_guard};

use borsh::de::BorshDeserialize;

use superposition_librfq::{Args, Imm, entry};

mod testing_addrs;

macro_rules! env_word {
    ($name:ident, $input:literal) => {
        #[cfg(feature = "testing")]
        const $name: U = testing_addrs::$name;
        #[cfg(not(feature = "testing"))]
        const $name: U = U::const_from_hex(env!($input).as_bytes()).unwrap();
    };
}

env_word!(SERVER_PUBLIC_KEY, "SPN_SERVER_PUBLIC_KEY");

#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_entrypoint(len: usize) -> usize {
    let args = read_args_vec(len);
    let imm = Imm {
        should_server_sig_validate: true,
        should_user_sig_validate: true,
        server_key: SERVER_PUBLIC_KEY,
    };
    flush_guard(|| entry(&imm, Args::deserialize(&mut args.as_slice()).unwrap()))
}

#[cfg(feature = "testing")]
fn main() {}
