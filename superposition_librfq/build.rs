use std::{env, fs, path::PathBuf, process::Command};

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => panic!("invalid hex digit: {:?}", b as char),
    }
}

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
    let commit_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
        .filter(|hash| hash.len() == 40);
    let (commit_doc, commit) = match commit_hash {
        Some(commit_hash) => {
            let bytes = commit_to_u_bytes(&commit_hash);

            let arr = bytes
                .iter()
                .map(|b| format!("0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ");

            (
                format!("Hash git commit {commit_hash}"),
                format!("U([{arr}])"),
            )
        }

        None => ("Git commit unavailable".to_string(), "U::MAX".to_string()),
    };

    let generated = format!(
        r#"use bobcat_sdk::maths::U;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {{
    /// {commit_doc}
    pub commit: U,
}}

impl Version {{
    pub const CURRENT: Self = Self {{
        commit: {commit},
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
