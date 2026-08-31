
use crate::{TOPIC_BALANCE_CHANGED, TOPIC_GLOBAL_BALANCE_CHANGED, Version};

use bobcat_sdk::{maths::U, events::emit};

pub fn event_emit_bal_update(acc_addr: [u8; 20], asset_addr: [u8; 20], user_bal: U, global_bal: U) {
    emit!(
        TOPIC_BALANCE_CHANGED,
        acc_addr,
        asset_addr,
        user_bal,
        data: Version::CURRENT,
        32
    );
    emit!(
        TOPIC_GLOBAL_BALANCE_CHANGED,
        asset_addr,
        global_bal,
        data: Version::CURRENT,
        32
    );
}
