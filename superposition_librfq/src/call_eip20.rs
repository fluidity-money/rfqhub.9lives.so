#[cfg(not(feature = "std"))]
pub use crate::wasm_eip20::*;

#[cfg(feature = "std")]
pub use crate::std_eip20::*;
