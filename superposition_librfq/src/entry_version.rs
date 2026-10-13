// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

use bobcat_sdk::{entry::write_result_word, maths::U};

use crate::version::Version;

pub fn entry_version() -> usize {
    write_result_word(&U::from(Version::CURRENT));
    0
}
