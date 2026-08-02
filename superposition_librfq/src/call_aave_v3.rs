use bobcat_sdk::{call::call_bool, interfaces::aave_v3::make_fn_supply, maths::U};

pub fn supply(addr: [u8; 20], amt: &U, receiver: [u8; 20], referrer: u16) -> bool {
    call_bool(
        addr,
        &make_fn_supply(addr, amt, receiver, referrer),
        &U::ZERO,
        u64::MAX,
    )
}
