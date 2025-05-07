use crate::merkle::CommitmentsAccount;
use crate::utils::serialize::{
    BorshDeserializeWithLength, BorshSerializeWithLength, DATA_LENGTH_CAPACITY,
};
use crate::{derive_pda, TREE_DEPTH};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program::invoke;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

// CommitmentsManagerAccount is a single account
// tracks all the commitments accounts by their tree number
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CommitmentsManagerAccount {
    pub incremental_tree_number: u64,
}

// initialize_commitments_manager create a new commiments manager account
// with an new commitments_account
//
// should only be call once when the contract is deployed
pub fn initialize_commitments_manager(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<(), ProgramError> {
    let accounts_iter: &mut std::slice::Iter<'_, _> = &mut accounts.iter();

    let payer_account = next_account_info(accounts_iter)?;
    let funding_account = next_account_info(accounts_iter)?;

    let commitments_account = next_account_info(accounts_iter)?;
    let commitments_manager_account = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, funding_bump_seed) =
        Pubkey::find_program_address(&[b"funding_pda"], program_id);
    // if funding_account.key != &funding_pda {
    //     return Err(ProgramError::InvalidSeeds);
    // }

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (commitments_manager_pda, commitments_manager_bump_seed) =
        Pubkey::find_program_address(&[b"commitments_manager_pda"], program_id);
    if commitments_manager_account.key != &commitments_manager_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // // account should only initialized once
    // if !commitments_manager_account.data_is_empty() {
    //     return Err(DarksolError::AccountAlreadyInitialized.into());
    // }

    // Derive the PDA for the newly account
    let (account_pda, bump_seed) = derive_pda(1, program_id);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Size of our commitments manager account
    let manager_account_space: usize = 8 + DATA_LENGTH_CAPACITY;

    // Calculate minimum balance for rent exemption
    let manager_account_rent = Rent::get()?;
    let manager_account_required_lamports =
        manager_account_rent.minimum_balance(manager_account_space);

    if funding_account.lamports() == 0 {
        invoke_signed(
            &system_instruction::create_account(
                payer_account.key,                 // Account paying for the new account
                &funding_pda,                      // Account to be created
                manager_account_required_lamports, // Amount of lamports to transfer to the new account
                0u64,                              // Size in bytes to allocate for the data field
                program_id,                        // Set program owner to our program
            ),
            &[
                payer_account.clone(),
                funding_account.clone(),
                system_program.clone(),
            ],
            &[&[b"funding_pda", &[funding_bump_seed]]],
        )?;
    }

    invoke(
        &system_instruction::transfer(payer_account.key, &funding_pda, 5_000_000_000),
        &[payer_account.clone(), funding_account.clone()],
    )?;

    // Create the commitments manager account
    invoke_signed(
        &system_instruction::create_account(
            &payer_account.key,                // Account paying for the new account
            &commitments_manager_pda,          // Account to be created
            manager_account_required_lamports, // Amount of lamports to transfer to the new account
            manager_account_space as u64,      // Size in bytes to allocate for the data field
            program_id,                        // Set program owner to our program
        ),
        &[
            payer_account.clone(),
            commitments_manager_account.clone(),
            system_program.clone(),
        ],
        &[&[b"commitments_manager_pda", &[commitments_manager_bump_seed]]],
    )?;

    // Size of our commitments account
    // set to maximum
    let account_space = 10240;

    // Calculate minimum balance for rent exemption
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(account_space);

    // Create the commitments account
    invoke_signed(
        &system_instruction::create_account(
            &payer_account.key,   // Account paying for the new account
            &account_pda,         // Account to be created
            required_lamports,    // Amount of lamports to transfer to the new account
            account_space as u64, // Size in bytes to allocate for the data field
            program_id,           // Set program owner to our program
        ),
        &[
            payer_account.clone(),
            commitments_account.clone(),
            system_program.clone(),
        ],
        &[&[&1u64.to_le_bytes(), &[bump_seed]]],
    )?;

    // Update incremental to 2 as we also create a new empty tree
    let new_manager_data = CommitmentsManagerAccount {
        incremental_tree_number: 1,
    };
    new_manager_data
        .serialize_with_length(&mut &mut commitments_manager_account.data.borrow_mut()[..])?;
    msg!(
        "creating new commitments manager account with increment: {}",
        2
    );

    // store empty tree to the newly created commitments account
    let new_empty_tree: CommitmentsAccount<TREE_DEPTH> = CommitmentsAccount::new(1);

    // Serialize the struct into the account's data
    new_empty_tree.serialize_with_length(&mut &mut commitments_account.data.borrow_mut()[..])?;

    msg!("commitments initialized");

    Ok(())
}

// initialize_commitments_account create a new commiments account
// to store a new tree. the address is derive from the program id
// the program id special derived account will be the payer. Update
// tree numbder increment
pub fn initialize_commitments_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<CommitmentsAccount<TREE_DEPTH>, ProgramError> {
    let accounts_iter: &mut std::slice::Iter<'_, _> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let commitments_account = next_account_info(accounts_iter)?;
    let commitments_mananger_account = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    if commitments_account.owner != program_id || commitments_mananger_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, funding_bump_seed) =
        Pubkey::find_program_address(&[b"funding_pda"], program_id);
    if funding_account.key != &funding_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // fetch the new tree number to derive newly account
    //
    // Mutable borrow the commitments manager account data
    let mut data = commitments_mananger_account.data.borrow_mut();
    // deserialize the data
    let mut manager_data: CommitmentsManagerAccount =
        CommitmentsManagerAccount::try_from_slice_with_length(&data)?;
    let new_tree_number = manager_data.incremental_tree_number + 1;

    // Derive the PDA for the newly account
    let (account_pda, _bump_seed) = derive_pda(new_tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Size of our commitments account
    // set to maximum
    let account_space = 10_485_760;

    // Calculate minimum balance for rent exemption
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(account_space);

    // Create the commitments account
    invoke_signed(
        &system_instruction::create_account(
            funding_account.key,  // Account paying for the new account
            &account_pda,         // Account to be created
            required_lamports,    // Amount of lamports to transfer to the new account
            account_space as u64, // Size in bytes to allocate for the data field
            program_id,           // Set program owner to our program
        ),
        &[
            funding_account.clone(),
            commitments_account.clone(),
            system_program.clone(),
        ],
        &[&[b"funding_pda", &[funding_bump_seed]]],
    )?;

    manager_data.incremental_tree_number = new_tree_number;
    // Serialize the CounterAccount struct into the account's data
    manager_data.serialize_with_length(&mut &mut data[..])?;

    msg!("adding new commitment account to manager");

    // store empty tree to the newly created commitments account
    let new_empty_tree: CommitmentsAccount<TREE_DEPTH> =
        CommitmentsAccount::new(new_tree_number as u64);
    // Serialize the struct into the account's data
    new_empty_tree.serialize_with_length(&mut &mut commitments_account.data.borrow_mut()[..])?;

    msg!("commitments initialized");

    Ok(new_empty_tree)
}
