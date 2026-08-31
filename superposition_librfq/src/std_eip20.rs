#[cfg(feature = "std")]
mod impls {
    use std::{cell::RefCell, collections::HashMap};

    use bobcat_sdk::{entry::contract_address, maths::U};

    pub type Address = [u8; 20];

    thread_local! {
        // Owner => asset => balance
        pub static BALANCES: RefCell<HashMap<Address, HashMap<Address, U>>> = RefCell::default();
    }

    pub fn reset_balances() {
        BALANCES.with(|s| s.borrow_mut().clear())
    }

    fn balance(asset: [u8; 20], owner: [u8; 20]) -> U {
        std_eip20::BALANCES.with(|balances| {
            balances
                .borrow()
                .get(&asset)
                .and_then(|owners| owners.get(&owner))
                .copied()
                .unwrap_or(U::ZERO)
        })
    }

    pub fn burn(asset: Address, spender: Address, amt: &U) {
        BALANCES
            .with(|b| -> Option<()> {
                let p = *b.borrow().get(&asset)?.get(&spender)?;
                *b.borrow_mut().get_mut(&asset)?.get_mut(&spender)? = p.checked_sub(amt);
                Some(())
            })
            .expect("went to burn {asset} for spender {spender}, no bal")
    }

    pub fn give(asset: Address, recipient: Address, amt: &U) {
        BALANCES.with(|b| {
            let mut b = b.borrow_mut();
            let b = match b.get_mut(&asset) {
                Some(b) => b,
                None => {
                    b.insert(asset, HashMap::new());
                    b.get_mut(&asset).unwrap()
                }
            };
            let existing_bal = match b.get_mut(&recipient) {
                None => U::ZERO,
                Some(v) => *v,
            };
            b.insert(recipient, existing_bal.checked_add(amt));
        });
    }

    pub fn safe_transfer_from(addr: [u8; 20], from: [u8; 20], to: [u8; 20], amt: &U) -> bool {
        burn(addr, from, amt);
        give(addr, to, amt);
        true
    }

    /// Transfer, checking the return type if we have it, but if we don't,
    /// we assume everything is okay.
    pub fn transfer(addr: [u8; 20], to: [u8; 20], amt: &U) -> bool {
        safe_transfer_from(addr, contract_address(), to, amt);
        true
    }
}

pub use impls::*;
