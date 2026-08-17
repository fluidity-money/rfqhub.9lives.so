use bobcat_sdk::maths::U;

#[derive(Debug, Clone)]
pub struct Imm {
    /// Should we validate any signatures that the server generated?
    pub should_server_sig_validate: bool,
    pub should_user_sig_validate: bool,
    pub addr_aave_v3: [u8; 20],
    pub server_key: U,
}

impl Default for Imm {
    fn default() -> Self {
        Self {
            should_server_sig_validate: false,
            should_user_sig_validate: false,
            addr_aave_v3: [0u8; 20],
            server_key: U::ZERO,
        }
    }
}
