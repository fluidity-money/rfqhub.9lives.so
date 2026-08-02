#![no_main]
#![no_std]

use bobcat_sdk::{entry::read_args_vec, storage::flush_guard};

use superposition_librfq::{Args, Imm, entry};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_entrypoint(len: usize) -> usize {
    let args = read_args_vec(len);
    flush_guard(|| entry(Args::deserialize(&mut args.as_slice()).unwrap()))
}
