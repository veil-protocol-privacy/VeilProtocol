use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo}, entrypoint, entrypoint::ProgramResult, instruction::{AccountMeta, Instruction}, program::invoke, program_error::ProgramError, pubkey::Pubkey
};
use veil_types::SP1Groth16Proof;

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let verification_program = next_account_info(accounts_iter)?;
    // Deserialize the SP1Groth16Proof from the instruction data.
    let groth16_proof = SP1Groth16Proof::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Create an instruction to invoke the DarkSol program.
    let instruction = Instruction::new_with_borsh(
        *verification_program.key,
        &groth16_proof,
        vec![],
    );

    // Invoke the DarkSol program.
    invoke(&instruction, &[verification_program.clone()])?;

    Ok(())
}