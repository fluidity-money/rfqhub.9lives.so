use crate::Outcome;

use superposition_assets::Asset;

use bobcat_sdk::maths::U;

macro_rules! storage {
    (
        $(
            $name:ident($($param:ident),*)
        ),*
        $(,)?
    ) => {
        storage! {
            @internal
            counter: 0,
            items: [$($name($($param),*)),*]
        }
    };

    (
        @internal
        counter: $counter:expr,
        items: []
    ) => {};

    (
        @internal
        counter: $counter:expr,
        items: [
            $name:ident($($param:ident),*)
            $(, $($rest:tt)*)?
        ]
    ) => {
        pub mod $name {
            pub(crate) use bobcat_sdk::storage::*;
            pub use crate::error::{Dis, Error};
            use bobcat_sdk::maths::{u, U};

            const SLOT: U = u!($counter);

            storage! {
                @impl
                params: [$($param),*]
            }
        }

        $(
            storage! {
                @internal
                counter: ($counter + 1),
                items: [$($rest)*]
            }
        )?
    };

    (
        @impl
        params: []
    ) => {
        pub fn get() -> U {
            storage_load(&SLOT)
        }

        pub fn set(value: &U) {
            storage_store(&SLOT, value);
        }

        pub fn checked_add_res(value: &U) -> Result<U, Error> {
            let current = get();
            let next =
                bobcat_sdk::maths::checked_add_opt(&current, value)
                    .ok_or(Dis::CheckedAddOverflow)?;
            set(&next);
            Ok(next)
        }

        pub fn checked_sub_res(value: &U) -> Result<U, Error> {
            let current = get();
            let next =
                bobcat_sdk::maths::checked_sub_opt(&current, value)
                    .ok_or(Dis::CheckedSubUnderflow)?;
            set(&next);
            Ok(next)
        }
    };

    (
        @impl
        params: [$($param:ident),+]
    ) => {
        fn storage_slot($($param: &U),+) -> U {
            storage! {
                @derive_slot
                slot: SLOT,
                keys: [$($param),+]
            }
        }

        pub fn get($($param: &U),+) -> U {
            let slot = storage_slot($($param),+);
            storage_load(&slot)
        }

        pub fn set($($param: &U,)+ value: &U) {
            let slot = storage_slot($($param),+);
            storage_store(&slot, value);
        }

        pub fn checked_add_res($($param: &U,)+ value: &U) -> Result<U, Error> {
            let slot = storage_slot($($param),+);
            let current = storage_load(&slot);
            let next =
                bobcat_sdk::maths::checked_add_opt(&current, value)
                    .ok_or(Error::from(Dis::CheckedAddOverflow))?;
            storage_store(&slot, &next);
            Ok(next)
        }

        pub fn checked_sub_res($($param: &U,)+ value: &U) -> Result<U, Error> {
            let slot = storage_slot($($param),+);
            let current = storage_load(&slot);
            let next =
                bobcat_sdk::maths::checked_sub_opt(&current, value)
                    .ok_or(Error::from(Dis::CheckedSubUnderflow))?;
            storage_store(&slot, &next);
            Ok(next)
        }

        pub fn exchange($($param: &U),+) {
            let slot = storage_slot($($param),+);
            assert!(
                storage_exchange_bool(&slot, false),
                "exchange failed"
            );
        }
    };

    (
        @derive_slot
        slot: $slot:expr,
        keys: [$key:ident $(, $rest:ident)*]
    ) => {{
        let slot = slot_map($key, &$slot);

        storage! {
            @derive_slot
            slot: slot,
            keys: [$($rest),*]
        }
    }};

    (
        @derive_slot
        slot: $slot:expr,
        keys: []
    ) => {
        $slot
    };
}

#[derive(Debug, Clone)]
pub struct WordBundleTakerDetails {
    pub amt: u64,
    pub expiry: [u8; 6],
    pub asset: Asset,
    pub price_target: u64,
    pub is_up: bool,
    pub outcome: Outcome,
}

pub struct BadAsset;

impl TryFrom<U> for WordBundleTakerDetails {
    type Error = BadAsset;

    fn try_from(U(bytes): U) -> Result<Self, Self::Error> {
        let amt = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let expiry: [u8; 6] = bytes[8..14].try_into().unwrap();
        let asset = Asset::try_from(bytes[20]).map_err(|_| BadAsset)?;
        let price_target = u64::from_le_bytes(bytes[21..29].try_into().unwrap());
        let is_up = bytes[29] != 0;
        let outcome = Outcome::from(bytes[30]);
        Ok(WordBundleTakerDetails {
            amt,
            expiry,
            asset,
            price_target,
            is_up,
            outcome,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WordBundleMakerDetails {
    pub amt: u64,
    pub acc: [u8; 20],
}

impl TryFrom<U> for WordBundleMakerDetails {
    type Error = BadAsset;

    fn try_from(U(bytes): U) -> Result<Self, Self::Error> {
        let amt = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let acc: [u8; 20] = bytes[8..28].try_into().unwrap();
        Ok(WordBundleMakerDetails { amt, acc })
    }
}

storage! {
    // The version number of this contract release.
    version(),
    // Global deposits from users.
    global_bal(asset),
    // Onramped balance of the account address given. Should be possible
    // to spend by the account address given here.
    onramped_bal(asset, account),
    // Taker amount that is available from the hash given.
    hash_taker_leftover(hash),
    // Address of the taker for the hash given.
    hash_taker_addr(hash),
    // Taker details from the hash given.
    hash_taker_details(hash),
    // The hash of the maker information that's associated with this taker.
    hash_ptr_maker_from_taker(hash),
    // The details of the hash for the maker.
    hash_maker_details(hash)
}
