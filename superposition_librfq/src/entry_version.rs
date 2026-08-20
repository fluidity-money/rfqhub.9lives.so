use bobcat_sdk::{entry::write_result_word, maths::U};

pub fn entry_version() -> usize {
    write_result_word(&U::ONE);
    0
}
