use solana_program::{
    account_info::AccountInfo, msg, program::invoke_signed, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, system_instruction, sysvar::Sysvar,
};

pub fn create_pda_account_from_pda_account<'a>(
    from_account: &AccountInfo<'a>,
    space: usize,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    new_pda_account: &AccountInfo<'a>,
    new_pda_signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let rent: &Rent = &Rent::get()?;

    let required_lamports = rent
        .minimum_balance(space)
        .max(1)
        .saturating_sub(from_account.lamports());

    msg!("Allocating PDA");
    invoke_signed(
        &system_instruction::allocate(new_pda_account.key, space.try_into().unwrap()),
        &[new_pda_account.clone(), system_program.clone()],
        &[new_pda_signer_seeds],
    )?;

    msg!("Assigning PDA");
    invoke_signed(
        &system_instruction::assign(new_pda_account.key, owner),
        &[new_pda_account.clone(), system_program.clone()],
        &[new_pda_signer_seeds],
    )?;

    msg!("Funding PDA");
    if required_lamports > 0 {
        **from_account.lamports.borrow_mut() -= required_lamports;
        **new_pda_account.lamports.borrow_mut() += required_lamports;
    }

    Ok(())
}

pub fn get_associated_token_address_and_bump_seed(
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
    program_id: &Pubkey,
    token_program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &wallet_address.to_bytes(),
            &token_program_id.to_bytes(),
            &token_mint_address.to_bytes(),
        ],
        program_id,
    )
}
