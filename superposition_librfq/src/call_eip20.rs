#[cfg(any(not(feature = "std"), target_arch = "wasm32"))]
pub use crate::wasm_eip20::*;

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
pub use crate::std_eip20::*;
