pub mod asset;
pub mod instruction;
pub mod merkle;
pub mod nullifier;
pub mod state;
pub mod entrypoint;
pub mod processor;

use primitive_types::U256;
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey
};

pub const ZERO_VALUE: U256 = U256([
    0x30644E72E131A029, 0xB85045B68181585D, 0x2833E84879B97091, 0x1A0111EA397FE69A,
]);

pub fn u256_to_bytes(value: U256) -> [u8; 32] {
    let mut bytes: [u8; 32] = [0u8; 32];
    value.to_big_endian(&mut bytes);
    bytes
}

pub fn derive_pda(value: u64, program_id: &Pubkey) -> (Pubkey, u8) {
    let seed = value.to_le_bytes();
    Pubkey::find_program_address(&[&seed], program_id)
}

pub fn is_account_initialized(account: &AccountInfo) -> bool {
    !account.data_is_empty() // Returns true if account has data
}