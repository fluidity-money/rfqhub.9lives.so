// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use crate::{TOPIC_BALANCE_CHANGED, TOPIC_GLOBAL_BALANCE_CHANGED, Version};

use bobcat_sdk::{events::emit, maths::U};

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
