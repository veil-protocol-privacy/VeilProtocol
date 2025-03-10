use borsh::{BorshDeserialize, BorshSerialize};
use solana_poseidon::hashv;
use solana_poseidon::{Endianness, Parameters, PoseidonHash};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use std::collections::HashMap;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UnshieldedAsset {
    assets: HashMap<Pubkey, u64>
}

impl UnshieldedAsset {
    pub fn update_assets(&mut self, addr: Pubkey, amount: u64) {
        if self.assets.contains_key(&addr) {
            // TODO: emit error
            let current_amt = self.assets.get(&addr).unwrap();
            let new_amount = current_amt + amount;
            self.assets.insert(addr, new_amount);
        } else {
            self.assets.insert(addr, amount);
        }

    }

    pub fn remove_assets(&mut self, addr: &Pubkey) {
        if self.assets.contains_key(addr) {
            self.assets.remove(addr);
        }
    }
}