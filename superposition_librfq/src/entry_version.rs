use bobcat_sdk::{entry::write_result_word, maths::U};

use crate::version::Version;

pub fn entry_version() -> usize {
    write_result_word(&U::from(Version::CURRENT));
    0
}
