pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod merkle;
pub mod processor;
pub mod state;

use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};
use std::clone;
use wasm_bindgen::prelude::*;

const TREE_DEPTH: usize = 32;

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

// PreCommitments contains info before being shielded inside protocol
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct PreCommitments {
    nullifier_pubkey: Vec<u8>, // Poseidon(Poseidon(spending public key, nullifying key), random)
    token_id: &Pubkey,
    value: u64,                     // amount
}

impl clone::Clone for PreCommitments {
    fn clone(&self) -> PreCommitments {
        return PreCommitments {
            nullifier_pubkey: self.encrypted_commitments.clone(),
            value: self.value.clone(),
            token_id: self.token_id.clone(),
        };
    }
}

// for js client support
#[wasm_bindgen]
impl PreCommitments {
    #[wasm_bindgen(constructor)]
    pub fn new(value: u64, token_id: &Pubkey, nullifier_pubkey: Vec<u8>) -> Self {
        PreCommitments {
            nullifier_pubkey,
            value,
            token_id,
        }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<PreCommitments, JsValue> {
        borsh::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
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
            shield_key: self.shield_key.clone(),
        };
    }
}

// for js client support
#[wasm_bindgen]
impl ShieldCipherText {
    #[wasm_bindgen(constructor)]
    pub fn new(shield_key: Vec<u8>) -> Self {
        ShieldCipherText {
            shield_key,
            encrypted_text: Vec::new(),
        }
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
    pub fn new(
        pre_commitments: PreCommitments,
        shield_cipher_text: ShieldCipherText,
        transfer_type: TransferType,
    ) -> Self {
        DepositRequest {
            pre_commitments,
            shield_cipher_text,
        }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<DepositRequest, JsValue> {
        borsh::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
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
    pub fn new(
        start_position: u64,
        tree_number: u64,
        pre_commitments: PreCommitments,
        shield_cipher_text: ShieldCipherText,
    ) -> Self {
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
        borsh::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct CommitmentCipherText {
    ciphertext: Vec<Vec<u8>>,
    encrypted_sender_key: Vec<u8>,
    encrypted_view_key: Vec<u8>,
    memo: Vec<u8>
}

impl clone::Clone for CommitmentCipherText {
    fn clone(&self) -> CommitmentCipherText {
        return CommitmentCipherText {
            ciphertext: self.ciphertext.clone(),
            encrypted_sender_key: self.encrypted_sender_key.clone(),
            encrypted_view_key: self.encrypted_view_key.clone(),
            memo: self.memo.clone(),
        };
    }
}

// for js client support
#[wasm_bindgen]
impl CommitmentCipherText {
    #[wasm_bindgen(constructor)]
    pub fn new(encrypted_sender_key: Vec<u8>, encrypted_view_key: Vec<u8>, memo: Vec<u8>) -> Self {
        CommitmentCipherText {
            encrypted_sender_key,
            encrypted_view_key,
            ciphertext: Vec::new(),
            memo,
        }
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
        self.ciphertext.push(value);
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct TransferRequest {
    proof: Vec<u8>,
    merkle_root: Vec<u8>,
    encrypted_commitments: Vec<Vec<u8>>, // list of newly generated commitments
    nullifiers: Vec<Vec<u8>>,            // nullifiers indicates spent UTXO
    metadata: RequestMetaData,
    commitment_cipher_text: Vec<CommitmentCipherText>,
}

#[wasm_bindgen]
impl TransferRequest {
    #[wasm_bindgen(constructor)]
    pub fn new(
        proof: Vec<u8>,
        merkle_root: Vec<u8>,
        tree_number: u64,
        commitment_cipher_text: Vec<CommitmentCipherText>,
    ) -> Self {
        TransferRequest {
            proof,
            merkle_root,
            encrypted_commitments: Vec::new(),
            nullifiers: Vec::new(),
            metadata: RequestMetaData::new(tree_number),
            commitment_cipher_text,
        }
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
    pub fn push_encrypted_commitments(&mut self, value: Vec<u8>) {
        self.encrypted_commitments.push(value);
    }

    #[wasm_bindgen]
    pub fn push_nullifiers(&mut self, value: Vec<u8>) {
        self.nullifiers.push(value);
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct RequestMetaData {
    tree_number: u64,
}

// for js client support
#[wasm_bindgen]
impl RequestMetaData {
    #[wasm_bindgen]
    pub fn new(tree_number: u64) -> Self {
        RequestMetaData { tree_number }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<DepositEvent, JsValue> {
        borsh::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}

// TransferEvent defines log after transfer instruction
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct TransferEvent {
    tree_number: u64,
    start_position: u64,
    commitments: Vec<Vec<u8>>,
    commitment_cipher_text: Vec<CommitmentCipherText>,
}

// for js client support
#[wasm_bindgen]
impl TransferEvent {
    #[wasm_bindgen]
    pub fn new(
        start_position: u64,
        tree_number: u64,
        commitment_cipher_text: Vec<CommitmentCipherText>,
    ) -> Self {
        TransferEvent {
            start_position,
            tree_number,
            commitments: Vec::new(),
            commitment_cipher_text,
        }
    }

    #[wasm_bindgen]
    pub fn from_js_value(js_value: JsValue) -> Result<TransferEvent, JsValue> {
        from_value(js_value).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn to_js_value(&self) -> Result<JsValue, JsValue> {
        to_value(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn push_data(&mut self, value: Vec<u8>) {
        self.commitments.push(value);
    }
}

#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct WithdrawRequest {
    proof: Vec<u8>,
    nullifiers: Vec<Vec<u8>>, // nullifiers indicates spent UTXO
    metadata: RequestMetaData,
    pre_commitments: PreCommitments,
}

#[wasm_bindgen]
impl WithdrawRequest {
    #[wasm_bindgen(constructor)]
    pub fn new(proof: Vec<u8>, tree_number: u64, amount: u64, token_id: &Pubkey) -> Self {
        WithdrawRequest {
            proof,
            nullifiers: Vec::new(),
            metadata: RequestMetaData::new(tree_number),
            pre_commitments: PreCommitments::new(amount, token_id, Vec::new()), // no need to provide the encrypted value here
            
        }
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
    pub fn push_nullifiers(&mut self, value: Vec<u8>) {
        self.nullifiers.push(value);
    }
}

pub fn fetch_mint_address(token_account: &AccountInfo) -> Result<String, ProgramError> {
    let token_data = token_account.try_borrow_data()?;
    let token_account = TokenAccount::unpack(&token_data)?;

    Ok(token_account.mint.to_string())
}

// WithdrawEvent defines log after withdraw instruction
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct WithdrawEvent {
    receiver: String,
    amount: u64,
    token_mint_address: String,
}

// for js client support
#[wasm_bindgen]
impl WithdrawEvent {
    #[wasm_bindgen]
    pub fn new(receiver: String, amount: u64, token_mint_address: String) -> Self {
        WithdrawEvent {
            receiver,
            amount,
            token_mint_address,
        }
    }

    #[wasm_bindgen]
    pub fn serialize(&self) -> Result<Vec<u8>, JsValue> {
        borsh::to_vec(self).map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize(data: &[u8]) -> Result<WithdrawEvent, JsValue> {
        borsh::from_slice(data)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))
    }
}

// NullifierEvent defines log after adding new nullifers instruction
#[wasm_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Debug, Serialize, Deserialize)]
pub struct NullifierEvent {
    nullifiers: Vec<Vec<u8>>,
}

// for js client support
#[wasm_bindgen]
impl NullifierEvent {
    #[wasm_bindgen]
    pub fn new() -> Self {
        NullifierEvent {
            nullifiers: Vec::new(),
        }
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
    pub fn push_nullifiers(&mut self, value: Vec<u8>) {
        self.nullifiers.push(value);
    }
}
