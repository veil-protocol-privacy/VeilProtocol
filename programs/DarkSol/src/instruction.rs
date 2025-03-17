use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

use crate::{DepositRequest, TransferRequest, WithdrawRequest};

// Instructions that our program can execute
#[derive(BorshSerialize, BorshDeserialize,Debug)]
pub enum DarkSolInstruction {
    Deposit {request: DepositRequest},
    Transfer {request: TransferRequest},
    Withdraw {request: WithdrawRequest},
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
                let request: DepositRequest = DepositRequest::try_from_slice(rest)?;
                Ok(Self::Deposit { request })
            }
            1 => {
                let request: TransferRequest = TransferRequest::try_from_slice(rest)?;
                Ok(Self::Transfer { request })
            } 
            2 => {
                let request: WithdrawRequest = WithdrawRequest::try_from_slice(rest)?;
                Ok(Self::Withdraw { request })
            } 
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
