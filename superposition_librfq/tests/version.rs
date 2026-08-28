use bobcat_sdk::maths::U;
use superposition_librfq::version::Version;

#[test]
fn generated_version_matches_cargo_package_version() {
    assert_eq!(Version::CRATE_VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn current_version_converts_to_version_word() {
    let version: U = Version::CURRENT.into();
    let word: [u8; 32] = version.0;

    assert_eq!(word[..31], [0; 31]);
    assert_eq!(word[31], 1);
}
