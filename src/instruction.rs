use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

// Instructions that our program can execute
#[derive(BorshSerialize, BorshDeserialize,Debug)]
pub enum DarkSolInstruction {
    Deposit {amount: u64},
    HideAssets {commitments: Vec<u8>},
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
                // For Deposit, parse a u64 from the remaining bytes for amount
                let amount = u64::from_le_bytes(
                    rest.try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(Self::Deposit { amount })
            }
            1 => {
                
                Ok(Self::HideAssets { commitments: rest.to_vec()})
            } 
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
