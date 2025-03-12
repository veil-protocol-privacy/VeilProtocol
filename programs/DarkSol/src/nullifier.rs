use borsh::{BorshSerialize, BorshDeserialize};
use std::collections::HashMap;
use solana_poseidon::{Endianness, Parameters, PoseidonHash};
use solana_poseidon::hashv;
use solana_program::pubkey::Pubkey;

// Define struct representing our nullifier manager account's data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct NullifierManagerAccount {
    nullifier_accounts: Vec<Pubkey>,
}

// Define struct representing our nullifier account's data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct NullifierAccount {
    nullifiers: HashMap<Vec<u8>, bool>,
}

impl NullifierAccount{
    fn new() -> Self {
        Self {
            nullifiers: HashMap::new(),
        }
    }

    fn insert_nullifier(
        &mut self,
        nullifier: Vec<u8>,
    ) {
        self.nullifiers.insert(nullifier, true);
    } 

    fn check_nullifier(
        &self,
        nullifier: Vec<u8>,
    ) -> bool {
        self.nullifiers.contains_key(&nullifier)
    }
    
}

// create a hash using user secret key and leaf commitment
fn create_nullifier(
    secret_key: &Vec<u8>,
    commitment: &Vec<u8>,
) -> Result<Vec<u8>, String> {
    let result: Result<PoseidonHash, solana_poseidon::PoseidonSyscallError> = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&secret_key, &commitment]);
    
    match result {
        Ok(hash) => {
            let bytes = hash.to_bytes();
            return Ok(bytes.to_vec());        
        }
        Err(err) => {
            return Err(format!("fail to create nullifier: {}", err.to_string()));
        }
    }
}

