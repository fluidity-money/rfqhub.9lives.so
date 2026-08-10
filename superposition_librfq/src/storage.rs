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

        pub fn checked_add(value: &U) {
            let current = get();
            let next = bobcat_sdk::maths::checked_add(&current, value);
            set(&next);
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

        pub fn checked_add($($param: &U,)+ value: &U) {
            let slot = storage_slot($($param),+);
            let current = storage_load(&slot);
            let next = bobcat_sdk::maths::checked_add(&current, value);
            storage_store(&slot, &next);
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
    global_bal(),
    // Onramped balance of the account address given. Should be possible
    // to spend by the account address given here.
    onramped_bal(account)
}
