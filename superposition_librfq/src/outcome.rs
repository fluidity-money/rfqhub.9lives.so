use borsh::{BorshDeserialize, BorshSerialize};

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Copy)]
pub enum Outcome {
    BitcoinPrice,
}

impl From<u8> for Outcome {
    fn from(_: u8) -> Self {
        Outcome::BitcoinPrice
    }
}
