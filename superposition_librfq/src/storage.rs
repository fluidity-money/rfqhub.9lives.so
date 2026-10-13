// Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
// source code is governed by a BUSL license that can be found in the
// LICENSE file.

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

            pub use crate::error::{Dis, Error, ErrorData};

            use alloc::boxed::Box;
            use bobcat_sdk::maths::{u, U};

            const SLOT: U = u!($counter);

            fn storage_error(
                dis: Dis,
                consuming_amt: U,
                prev_amt: U,
                hint: &'static str,
            ) -> Error {
                Error {
                    dis,
                    data: Box::new(ErrorData {
                        storage: Some(stringify!($name)),
                        consuming_amt: Some(consuming_amt),
                        prev_amt: Some(prev_amt),
                        hint: Some(hint),
                        ..ErrorData::default()
                    }),
                }
            }

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

        pub fn checked_add_res(
            value: &U,
            hint: &'static str,
        ) -> Result<U, Error> {
            let current = get();
            let next =
                bobcat_sdk::maths::checked_add_opt(
                    &current,
                    value,
                )
                .ok_or_else(|| {
                    storage_error(
                        Dis::CheckedAddOverflow,
                        *value,
                        current,
                        hint,
                    )
                })?;
            set(&next);
            Ok(next)
        }

        pub fn checked_sub_res(
            value: &U,
            hint: &'static str,
        ) -> Result<U, Error> {
            let current = get();
            let next =
                bobcat_sdk::maths::checked_sub_opt(
                    &current,
                    value,
                )
                .ok_or_else(|| {
                    storage_error(
                        Dis::CheckedSubUnderflow,
                        *value,
                        current,
                        hint,
                    )
                })?;
            set(&next);
            Ok(next)
        }
    };
    (
        @impl
        params: [$($param:ident),+]
    ) => {
        fn storage_slot(
            $($param: &U),+
        ) -> U {
            storage! {
                @derive_slot
                slot: SLOT,
                keys: [$($param),+]
            }
        }

        pub fn get(
            $($param: &U),+
        ) -> U {
            let slot = storage_slot($($param),+);
            storage_load(&slot)
        }

        pub fn set(
            $($param: &U,)+
            value: &U
        ) {
            let slot = storage_slot($($param),+);
            storage_store(
                &slot,
                value,
            );
        }

        pub fn checked_add_res(
            $($param: &U,)+
            value: &U,
            hint: &'static str,
        ) -> Result<U, Error> {
            let slot = storage_slot($($param),+);
            let current = storage_load(&slot);
            let next =
                bobcat_sdk::maths::checked_add_opt(
                    &current,
                    value,
                )
                .ok_or_else(|| {
                    storage_error(
                        Dis::CheckedAddOverflow,
                        *value,
                        current,
                        hint,
                    )
                })?;
            storage_store(&slot, &next);
            Ok(next)
        }

        pub fn checked_sub_res(
            $($param: &U,)+
            value: &U,
            hint: &'static str,
        ) -> Result<U, Error> {
            let slot = storage_slot($($param),+);
            let current = storage_load(&slot);
            let next =
                bobcat_sdk::maths::checked_sub_opt(
                    &current,
                    value,
                )
                .ok_or_else(|| {
                    storage_error(
                        Dis::CheckedSubUnderflow,
                        *value,
                        current,
                        hint,
                    )
                })?;
            storage_store(&slot, &next);
            Ok(next)
        }

        pub fn exchange_res(
            $($param: &U),+,
            hint: &'static str
        ) -> Result<(), Error> {
            let slot = storage_slot($($param),+);
            storage_exchange_res(&slot, &U::ZERO, &U::ONE).map_err(|o| {
                Error {
                    dis: Dis::ExchangeErr,
                    data: Box::new(ErrorData {
                        hash: None,
                        storage: Some(hint),
                        old_val: Some(o),
                        hint: Some(hint),
                        ..ErrorData::default()
                    }),
                }
            })
        }
    };

    (
        @derive_slot
        slot: $slot:expr,
        keys: [
            $key:ident
            $(, $rest:ident)*
        ]
    ) => {{
        let slot =
            slot_map(
                $key,
                &$slot,
            );
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
    // Was this paused due to some issue?
    paused(),
    // The version number of this contract release.
    version(),
    // Global deposits from users.
    global_bal(asset),
    // Onramped balance of the account address given. Should be possible
    // to spend by the account address given here.
    onramped_bal(asset, account),
    // Seen timestamps that we track to prevent reuse.
    ms_ts(account, ms_ts),
    // Seen server timestamps that we track to prevent reuse. Keyed by
    // the account we intended this action for, and the timestamp.
    server_ms_ts(acc_addr, ms_ts),
    // Taker amount that is available to be withdrawn for the taker of this
    // position:
    hash_taker_leftover(hash, asset),
    // Maker amount that is available to be potentially withdrawn for the
    // maker of this position.
    hash_maker_leftover(hash, asset),
    // Address of the taker for the hash given.
    hash_taker_addr(hash),
    // Address of the maker for the hash given.
    hash_maker_addr(hash),
    // The asset in use for the hash given.
    hash_asset(hash)
}
