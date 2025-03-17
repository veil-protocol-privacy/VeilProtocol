use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, pubkey::Pubkey,
};
use crate::instruction::DarkSolInstruction;
use crate::processor::{process_deposit_fund, process_transfer_asset, process_withdraw_asset};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Unpack instruction data
    let instruction = DarkSolInstruction::unpack(instruction_data)?;

    // Match instruction type
    match instruction {
        DarkSolInstruction::Deposit { request } => {
            process_deposit_fund(program_id, accounts, request)?
        }
        DarkSolInstruction::Transfer { request } => process_transfer_asset(program_id, accounts, request)?,
        DarkSolInstruction::Withdraw { request } => process_withdraw_asset(program_id, accounts, request)?,
    };
    Ok(())
}