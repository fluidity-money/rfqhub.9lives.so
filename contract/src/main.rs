#![no_main]
#![no_std]

use bobcat_sdk::{entry::read_args_vec, storage::flush_guard};

use borsh::de::BorshDeserialize;

use superposition_librfq::{Args, Imm, entry};

macro_rules! env_addr {
    ($name:ident, $input:literal) => {
        const $name: [u8; 20] =
            match const_hex::const_decode_to_array::<20>(env!($input).as_bytes()) {
                Ok(res) => res,
                Err(_) => panic!(),
            };
    };
}

env_addr!(ADDR_FUSDC, "SPN_FUSDC_ADDR");

#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_entrypoint(len: usize) -> usize {
    let args = read_args_vec(len);
    let imm = Imm {
        addr_fusdc: ADDR_FUSDC,
        addr_aave_v3: [0u8; 20],
    };
    flush_guard(|| entry(&imm, Args::deserialize(&mut args.as_slice()).unwrap()))
}
