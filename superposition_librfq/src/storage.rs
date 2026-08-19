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

storage! {
    // The version number of this contract release.
    version(),
    // Global deposits from users.
    global_bal(asset),
    // Onramped balance of the account address given. Should be possible
    // to spend by the account address given here.
    onramped_bal(asset, account),
    // Taker amount that is available from the hash given.
    hash_taker_leftover(hash, asset),
    // Maker amount that is available from the hash given.
    hash_maker_leftover(hash, asset),
    // Address of the taker for the hash given.
    hash_taker_addr(hash),
    // The asset in use for the hash given.
    hash_asset(hash)
}
