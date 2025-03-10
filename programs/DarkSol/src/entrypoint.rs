use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, pubkey::Pubkey,
};
use crate::instruction::DarkSolInstruction;
use crate::processor::{process_deposit_fund, process_hide_asset, process_transfer_asset, process_withdraw_asset};

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
        DarkSolInstruction::Deposit { amount } => {
            process_deposit_fund(program_id, accounts, amount)?
        }
        DarkSolInstruction::HideAssets {commitments } => {
            process_hide_asset(program_id, accounts,commitments )?
        }
        DarkSolInstruction::Transfer { proofs } => process_transfer_asset(program_id, accounts, proofs)?,
        DarkSolInstruction::Withdraw { proofs } => process_withdraw_asset(program_id, accounts, proofs)?,
    };
    Ok(())
}