use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

use crate::{DepositRequest, deserialize_requests};

// Instructions that our program can execute
#[derive(BorshSerialize, BorshDeserialize,Debug)]
pub enum DarkSolInstruction {
    Deposit {requests: Vec<DepositRequest>},
    Transfer {proofs: Vec<u8>},
    Withdraw {proofs: Vec<u8>},
}

impl DarkSolInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        // Get the instruction variant from the first byte
        let (&variant, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        // Match instruction type and parse the remaining bytes based on the variant
        match variant {
            0 => {
                let requests = deserialize_requests(rest)?;
                Ok(Self::Deposit { requests })
            }
            1 => {
                Ok(Self::Transfer { proofs: rest.to_vec() })
            } 
            2 => {
                Ok(Self::Withdraw { proofs: rest.to_vec() })
            } 
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
