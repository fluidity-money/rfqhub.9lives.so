use std::{env, fs, path::PathBuf, process::Command};

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => panic!("invalid hex digit: {:?}", b as char),
    }
}

/// Turn a 40-char git commit hash into a big-endian [u8; 32] U word.
/// The hash is 20 bytes, left-padded with 12 zero bytes to fill the 256-bit word.
fn commit_to_u_bytes(hex: &str) -> [u8; 32] {
    let nibbles = hex.as_bytes();
    assert_eq!(
        nibbles.len(),
        40,
        "git commit hash must be 40 hex chars, got {:?}",
        hex
    );
    let mut arr = [0u8; 32];
    for i in 0..20 {
        arr[12 + i] = (hex_val(nibbles[2 * i]) << 4) | hex_val(nibbles[2 * i + 1]);
    }
    arr
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // The git commit hash the contract is compiled from.
    let commit_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("failed to run `git rev-parse HEAD`");
    assert!(
        commit_hash.status.success(),
        "`git rev-parse HEAD` failed: {}",
        String::from_utf8_lossy(&commit_hash.stderr)
    );
    let commit_hash = String::from_utf8(commit_hash.stdout)
        .expect("git output is not valid UTF-8")
        .trim()
        .to_string();

    let bytes = commit_to_u_bytes(&commit_hash);
    let arr = bytes
        .iter()
        .map(|b| format!("0x{b:02x}"))
        .collect::<Vec<_>>()
        .join(", ");

    let generated = format!(
        r#"use bobcat_sdk::maths::U;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {{
    /// The git commit hash (as a single U word) this contract was compiled from: {commit_hash}
    pub commit: U,
}}

impl Version {{
    pub const CURRENT: Self = Self {{
        commit: U([{arr}]),
    }};
}}

impl From<Version> for U {{
    fn from(v: Version) -> Self {{
        v.commit
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
