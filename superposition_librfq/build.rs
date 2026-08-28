use std::{env, fs, path::PathBuf};

const VERSION_TAG: u8 = 1;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");

    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by Cargo");
    let variant = format!(
        "V{}",
        version
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>()
    );

    let generated = format!(
        r#"use bobcat_sdk::maths::U;

/// Versions of the RFQ hub crate and their externally visible tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Version {{
    /// Crate version {version}.
    {variant} = {VERSION_TAG},
}}

impl Version {{
    pub const CURRENT: Self = Self::{variant};
    pub const CRATE_VERSION: &'static str = "{version}";
}}

impl From<Version> for U {{
    fn from(v: Version) -> Self {{
        Self::from(v as u8).into()
    }}
}}

impl From<Version> for [u8; 32] {{
    fn from(v: Version) -> Self {{
        U::from(v).into()
    }}
}}
"#
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    fs::write(out_dir.join("version.rs"), generated).expect("write generated version module");
}
