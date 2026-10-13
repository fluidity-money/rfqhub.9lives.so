// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

#[cfg(target_arch = "wasm32")]
pub use crate::wasm_eip20::*;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::std_eip20::*;
