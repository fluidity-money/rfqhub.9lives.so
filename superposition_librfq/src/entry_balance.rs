// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use bobcat_sdk::entry::write_result_word;

use crate::storage;

pub fn entry_balance(asset: [u8; 20], addr: [u8; 20]) -> usize {
    write_result_word(&storage::onramped_bal::get(&asset.into(), &addr.into()));
    0
}
