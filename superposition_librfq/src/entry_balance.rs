use bobcat_sdk::{maths::U, entry::write_result_word};

use crate::storage;

pub fn entry_balance(addr: [u8; 20]) -> usize {
    write_result_word(&storage::onramped_bal::get(&U::from(addr)));
    0
}
