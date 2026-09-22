#[cfg(target_arch = "wasm32")]
pub use crate::wasm_eip20::*;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::std_eip20::*;
