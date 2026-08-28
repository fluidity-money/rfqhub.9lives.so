use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    let v = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by Cargo");
    let [major, minor, patch] = v.split(".").collect::<Vec<&str>>()[..].try_into().unwrap();
    let generated = format!(
        r#"use bobcat_sdk::maths::U;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {{
    pub major: u64,
    pub minor: u64,
    pub patch: u64
}}

impl Version {{
    pub const CURRENT: Self = Self {{
        major: {major},
        minor: {minor},
        patch: {patch},
    }};
}}

impl From<Version> for U {{
    fn from(Version {{ major, minor, patch }}: Version) -> Self {{
        let mut x = U::ZERO;
        x |= U::from(major) << 128;
        x |= U::from(minor) << 64;
        x |= patch.into();
        x
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
