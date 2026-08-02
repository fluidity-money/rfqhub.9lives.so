use bobcat_sdk::{call::safe_call_slice, interfaces::eip20::make_fn_transfer_from, maths::U};

pub fn transfer_from(addr: [u8; 20], from: [u8; 20], to: [u8; 20], amt: &U) -> bool {
    // If we don't have code, or the length is 1 but it's false, then the
    // transferFrom failed!
    if let Some((true, len, rd)) = safe_call_slice::<1>(
        addr,
        &make_fn_transfer_from(from, to, &amt),
        &U::ZERO,
        u64::MAX,
        1,
    ) {
        len == 0 || rd[0] == 1
    } else {
        false
    }
}
