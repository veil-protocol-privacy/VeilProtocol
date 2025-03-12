pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod merkle;
pub mod nullifier;
pub mod processor;
pub mod state;

use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::U256;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey, program_error::ProgramError};
use wasm_bindgen::prelude::*;
use std::clone;

pub const ZERO_VALUE: U256 = U256([
    0x30644E72E131A029,
    0xB85045B68181585D,
    0x2833E84879B97091,
    0x1A0111EA397FE69A,
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

// PreCommitments contains info before being shielded inside protocol
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PreCommitments {
    encrypted_commitments: Vec<u8>, // Poseidon(Poseidon(spending public key, nullifying key), random)
    value: u64,                     // amount
}

impl clone::Clone for PreCommitments {
    fn clone(&self) -> PreCommitments {
        return PreCommitments {
            encrypted_commitments: self.encrypted_commitments.clone(),
            value: self.value.clone()
        };
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DepositRequest {
    pre_commitments: PreCommitments,
    commitments: Vec<u8>,
}

pub fn deserialize_requests(input: &[u8]) -> Result<Vec<DepositRequest>, ProgramError> {
    Vec::<DepositRequest>::try_from_slice(input).map_err(|_| ProgramError::InvalidInstructionData)
}