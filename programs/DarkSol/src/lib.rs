pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod merkle;
pub mod nullifier;
pub mod processor;
pub mod state;

use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::U256;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
use wasm_bindgen::prelude::*;
use std::clone;
use serde::{Serialize, Deserialize};
use serde_wasm_bindgen::{from_value, to_value};

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

// for js client support 
#[wasm_bindgen]
impl PreCommitments {
    #[wasm_bindgen(constructor)]
    pub fn new(value: u64, encrypted_commitments: Vec<u8>) -> Self {
        PreCommitments { encrypted_commitments, value }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<PreCommitments, JsValue> {
        borsh::from_slice(data).map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct ShieldCipherText {
    encrypted_text: Vec<Vec<u8>>,
    shield_key: Vec<u8>,
}

impl clone::Clone for ShieldCipherText {
    fn clone(&self) -> ShieldCipherText {
        return ShieldCipherText {
            encrypted_text: self.encrypted_text.clone(),
            shield_key: self.shield_key.clone()
        };
    }
}

// for js client support 
#[wasm_bindgen]
impl ShieldCipherText {
    #[wasm_bindgen(constructor)]
    pub fn new(shield_key: Vec<u8>, ) -> Self {
        ShieldCipherText { shield_key, encrypted_text: Vec::new() }
    }

    #[wasm_bindgen]
    pub fn from_js_value(js_value: JsValue) -> Result<ShieldCipherText, JsValue> {
        from_value(js_value).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn to_js_value(&self) -> Result<JsValue, JsValue> {
        to_value(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn push_data(&mut self, value: Vec<u8>) {
        self.encrypted_text.push(value);
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DepositRequest {
    pre_commitments: PreCommitments,
    shield_cipher_text: ShieldCipherText,
}

// for js client support 
#[wasm_bindgen]
impl DepositRequest {
    #[wasm_bindgen(constructor)]
    pub fn new(pre_commitments: PreCommitments, shield_cipher_text: ShieldCipherText) -> Self {
        DepositRequest { pre_commitments, shield_cipher_text }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<DepositRequest, JsValue> {
        borsh::from_slice(data).map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}

// DepositEvent defines log after deposit instruction
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DepositEvent {
    tree_number: u64,
    start_position: u64,
    pre_commitments: PreCommitments,
    shield_cipher_text: ShieldCipherText,
}

// for js client support 
#[wasm_bindgen]
impl DepositEvent {
    #[wasm_bindgen]
    pub fn new(start_position: u64, tree_number: u64, pre_commitments: PreCommitments, shield_cipher_text: ShieldCipherText,) -> Self {
        DepositEvent {
            start_position,
            tree_number,
            pre_commitments,
            shield_cipher_text,
        }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<DepositEvent, JsValue> {
        borsh::from_slice(data).map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}