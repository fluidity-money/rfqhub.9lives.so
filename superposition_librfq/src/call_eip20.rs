use bobcat_sdk::{
    call::{call_slice, safe_call_slice},
    interfaces::eip20::{make_fn_transfer, make_fn_transfer_from},
    maths::U,
};

pub fn safe_transfer_from(addr: [u8; 20], from: [u8; 20], to: [u8; 20], amt: &U) -> bool {
    // If we don't have code, or the length is 1 but it's false, then the
    // transferFrom failed!
    if let Some((true, len, rd)) = safe_call_slice::<1>(
        addr,
        &make_fn_transfer_from(from, to, &amt),
        &U::ZERO,
        u64::MAX,
        31,
    ) {
        len == 0 || rd[0] == 1
    } else {
        false
    }
}

/// Transfer, checking the return type if we have it, but if we don't,
/// we assume everything is okay.
pub fn transfer(addr: [u8; 20], to: [u8; 20], amt: &U) -> bool {
    if let (true, len, rd) =
        call_slice::<1>(addr, &make_fn_transfer(to, &amt), &U::ZERO, u64::MAX, 31)
    {
        len == 0 || rd[0] == 1
    } else {
        false
    }
}
