// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use bobcat_sdk::maths::U;

#[derive(Debug, Clone)]
pub struct Imm {
    /// Should we validate any signatures that the server generated?
    pub should_server_sig_validate: bool,
    pub should_user_sig_validate: bool,
    pub server_key: U,
}

impl Default for Imm {
    fn default() -> Self {
        Self {
            should_server_sig_validate: false,
            should_user_sig_validate: false,
            server_key: U::ZERO,
        }
    }
}
