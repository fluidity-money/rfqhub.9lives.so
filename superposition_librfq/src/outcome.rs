use borsh::{BorshDeserialize, BorshSerialize};

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Copy)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "proptest", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Outcome {
    BitcoinPrice,
}

impl From<u8> for Outcome {
    fn from(_: u8) -> Self {
        Outcome::BitcoinPrice
    }
}

#[test]
fn test_print_outcome() {
    serde_json::to_writer(&std::io::stdout(), &Outcome::BitcoinPrice).unwrap();
    panic!("shit")
}