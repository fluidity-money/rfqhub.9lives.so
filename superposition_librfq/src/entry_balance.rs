use bobcat_sdk::entry::write_result_word;

use crate::storage;

pub fn entry_balance(asset: [u8; 20], addr: [u8; 20]) -> usize {
    write_result_word(&storage::onramped_bal::get(&asset.into(), &addr.into()));
    0
}
