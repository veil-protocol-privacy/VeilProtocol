use crate::merkle::CommitmentsAccount;
use std::collections::HashMap;
use crate::{derive_pda, fetch_mint_address, DepositEvent, DepositRequest, NullifierEvent, TransferEvent, TransferRequest, WithdrawEvent, WithdrawRequest};
use crate::{
    error::DarksolError,
    merkle::hash_precommits,
    state::{initialize_commitments_account, CommitmentsManagerAccount},
    TREE_DEPTH,
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::log::sol_log_data;
use solana_program::program::invoke;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::{
    instruction::{initialize_account, transfer as spl_transfer},
    state::Account as TokenAccount,
    solana_program::program_pack::Pack,
};

// transfer_token_in deposit user fund into contract owned account.
// Create a new token account for the deposit account if it's not initialized yet
fn transfer_token_in(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let accounts_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let mint_account = next_account_info(accounts_iter)?; // SPL Token Mint
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program
    let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], program_id);
    if funding_account.key != &funding_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // TODO: apply deposit fee

    // Check if PDA's token account is already initialized
    if pda_token_account.data_is_empty() {
        // PDA's token account is not initialized → Create it

        let rent: &Rent = &Rent::get()?;
        let required_lamports = rent.minimum_balance(TokenAccount::LEN);

        invoke_signed(
            &system_instruction::create_account(
                &funding_pda, // funding account pays for the new account
                pda_token_account.key,
                required_lamports,
                TokenAccount::LEN as u64,
                token_program.key,
            ),
            &[
                user_wallet.clone(),
                pda_token_account.clone(),
                system_program.clone(),
            ],
            &[&[b"funding_pda", &[bump_seed]]], // PDA signs
        )?;

        invoke_signed(
            // TODO: emit error
            &initialize_account(
                token_program.key,
                pda_token_account.key,
                mint_account.key,
                &funding_account.key, // PDA is the owner of this token account
            )?,
            &[
                pda_token_account.clone(),
                mint_account.clone(),
                funding_account.clone(),
                token_program.clone(),
            ],
            &[&[b"funding_pda", &[bump_seed]]], // PDA signs
        )?;
    }

    // transfer token to contract owned token account
    invoke(
        // TODO: emit error
        &spl_transfer(
            token_program.key,
            user_token_account.key,
            pda_token_account.key,
            user_wallet.key, // User must sign as authority
            &[],
            amount,
        )?,
        &[
            user_token_account.clone(),
            pda_token_account.clone(),
            user_wallet.clone(),
            token_program.clone(),
        ],
    )?;

    Ok(())
}

fn transfer_token_out(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let accounts_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], program_id);

    // check all the accounts info
    if funding_account.key != &funding_pda || pda_token_account.owner != &funding_pda || user_token_account.owner != user_wallet.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // TODO: apply withdraw fee

    // transfer token from contract owned token account to user token address
    invoke_signed(
        // TODO: emit error
        &spl_transfer(
            token_program.key,
            pda_token_account.key,
            user_token_account.key,
            user_wallet.key, // User must sign as authority
            &[],
            amount,
        )?,
        &[
            pda_token_account.clone(),
            user_token_account.clone(),
            user_wallet.clone(),
            token_program.clone(),
        ],
        &[&[b"funding_pda", &[bump_seed]]], // PDA signs
    )?;

    Ok(())
}


// process_deposit_fund deposit user fund into contract owned account
// insert new UTXO into current merkel tree, if exceeds maximum tree depth
// create new account to store new tree
pub fn process_deposit_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: DepositRequest,
) -> ProgramResult {
    let accounts_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let mint_account: &AccountInfo<'_> = next_account_info(accounts_iter)?; // SPL Token Mint
    let commitments_account = next_account_info(accounts_iter)?; // current commitments account
    let commitments_manager_account = next_account_info(accounts_iter)?;
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program
    let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts

    if commitments_account.owner != program_id || commitments_manager_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // fetch the current tree number
    let data = commitments_manager_account.data.borrow_mut();
    // deserialize the data
    let manager_data: CommitmentsManagerAccount = CommitmentsManagerAccount::try_from_slice(&data)?;

    // Derive the PDA for the current commitments account
    let (account_pda, _bump_seed) = derive_pda(manager_data.incremental_tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // transfer token to contract owned account
    transfer_token_in(
        program_id,
        &[
            funding_account.clone(),
            user_wallet.clone(),
            user_token_account.clone(),
            pda_token_account.clone(),
            mint_account.clone(),
            token_program.clone(),
            system_program.clone(),
        ],
        request.pre_commitments.value,
    )?;

    let inserted_leaf = hash_precommits(request.pre_commitments.clone());

    // fetch current tree number
    let mut commitments_data = &mut commitments_account.data.borrow_mut()[..];
    // deserialize the data
    let mut current_tree = CommitmentsAccount::try_from_slice(&commitments_data)?;
    let mut current_tree_number: u64 = manager_data.incremental_tree_number;
    let start_position: u64 = 0;

    // create new commitments account if insert leaf exceeds max tree depth
    // user should check if the inserted leafs exceeds max tree depth to
    // add new commitments account to the instruction
    if current_tree.exceed_tree_depth(1) {
        let new_commitments_account = next_account_info(accounts_iter)?; // new commitments account

        // derive a new commitments account and update the commitments account
        let (new_pda, _bump_seed) = derive_pda(current_tree_number + 1, program_id);

        if new_commitments_account.key != &new_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        current_tree = initialize_commitments_account(
            program_id,
            &[
                funding_account.clone(),
                new_commitments_account.clone(),
                commitments_manager_account.clone(),
                system_program.clone(),
            ],
        )?;

        let mut new_commitments_data = &mut new_commitments_account.data.borrow_mut()[..];
        current_tree_number += 1;

        // insert leaf into tree
        let result = current_tree.insert_commitments(&mut vec![inserted_leaf.clone()]);
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data.serialize(&mut new_commitments_data)?
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }
    } else {
        // insert leaf into tree
        let result = current_tree.insert_commitments(&mut vec![inserted_leaf.clone()]);
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data.serialize(&mut commitments_data)?
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }
    }

    // emit events for indexer to scan
    let event = DepositEvent {
        start_position,
        tree_number: current_tree_number,
        pre_commitments: request.pre_commitments.clone(),
        shield_cipher_text: request.shield_cipher_text.clone(),
    };
    let serialize_event = borsh::to_vec(&event)?;
    sol_log_data(&[b"deposit_event", &serialize_event]);

    Ok(())
}

// process_transfer_asset takes the zkp for ownership of the UTXO
// in the merkel tree, check the nullifier for that UTXO is marked on the list or not
// and inserts new encrypted UTXO commitments to the merkle tree
pub fn process_transfer_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: TransferRequest,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let commitments_account = next_account_info(accounts_iter)?; // current commitments account
    let commitments_manager_account = next_account_info(accounts_iter)?;

    if commitments_account.owner != program_id || commitments_manager_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive the PDA for the commitments account
    let (account_pda, _bump_seed) = derive_pda(request.metadata.tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    let mut commitments_acc_data = &mut commitments_account.data.borrow_mut()[..];
    let mut inserted_tree: CommitmentsAccount<TREE_DEPTH> =
        CommitmentsAccount::try_from_slice(&commitments_acc_data)?;

    let mut inserted_nullifiers: HashMap<Vec<u8>, bool> = HashMap::new();

    // TODO: verify proof

    // ------------------- verify logic end here ------------------------ //

    // check if nullifiers already exists if not added to the list
    for idx in 0..request.nullifiers.len() {
        if inserted_tree.check_nullifier(&request.nullifiers[idx]) || inserted_nullifiers.contains_key(&request.nullifiers[idx]) {
            return Err(DarksolError::UtxoAlreadySpent.into());
        }

        inserted_nullifiers.insert(request.nullifiers[idx].clone(), true);
    }

    // fetch the current tree number
    let data = commitments_manager_account.data.borrow_mut();
    // deserialize the data
    let manager_data: CommitmentsManagerAccount = CommitmentsManagerAccount::try_from_slice(&data)?;
    let mut current_tree_number = manager_data.incremental_tree_number;

    let start_position: u64;

    // update merkle tree
    // create new commitments account if insert leaf exceeds max tree depth
    // user should check if the inserted leafs exceeds max tree depth to
    // add new commitments account, funding account and system program to the instruction
    if inserted_tree.exceed_tree_depth(request.encrypted_commitments.len()) {
        let funding_account = next_account_info(accounts_iter)?;
        let new_commitments_account = next_account_info(accounts_iter)?; // new commitments account
        let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts

        current_tree_number += 1;
        // derive a new commitments account and update the commitments account
        let (new_pda, _bump_seed) = derive_pda(current_tree_number, program_id);

        if new_commitments_account.key != &new_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        inserted_tree = initialize_commitments_account(
            program_id,
            &[
                funding_account.clone(),
                new_commitments_account.clone(),
                commitments_manager_account.clone(),
                system_program.clone(),
            ],
        )?;

        // insert nullifiers to newly created tree
        inserted_tree.insert_nullifiers(inserted_nullifiers);
        let mut new_commitments_data = &mut new_commitments_account.data.borrow_mut()[..];

        // insert leaf into tree
        let result = inserted_tree.insert_commitments(&mut request.encrypted_commitments.clone());
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data.serialize(&mut new_commitments_data)?;

                start_position = resp.commitments_data.next_leaf_index as u64;
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }

    } else {
        inserted_tree.insert_nullifiers(inserted_nullifiers);
        // insert leaf into tree
        let result = inserted_tree.insert_commitments(&mut request.encrypted_commitments.clone());
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data.serialize(&mut commitments_acc_data)?;

                start_position = resp.commitments_data.next_leaf_index as u64;
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }
    }

    // emit event
    let event = TransferEvent {
        start_position,
        tree_number: current_tree_number,
        commitments: request.encrypted_commitments.clone(),
        commitment_cipher_text: request.commitment_cipher_text.clone(),
    };
    let serialize_event = borsh::to_vec(&event)?;
    sol_log_data(&[b"transfer_event", &serialize_event]);

    let nullifier_event = NullifierEvent {
        nullifiers: request.nullifiers.clone(),
    };
    let nullifier_serialize_event = borsh::to_vec(&nullifier_event)?;
    sol_log_data(&[b"nullifiers_event", &nullifier_serialize_event]);

    Ok(())
}

// process_withdraw_asset verifies the zkp for ownership of the UTXO
// transfer the token from contract vault token account to receiver
// token account
pub fn process_withdraw_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: WithdrawRequest,
) -> ProgramResult {
    let accounts_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let commitments_account = next_account_info(accounts_iter)?; // current commitments account
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program

    if commitments_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    // TODO: verify proof

    // ------------------ verify logic end ---------------------- //

    // Derive the PDA for the commitments account
    let (account_pda, _bump_seed) = derive_pda(request.metadata.tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    let mut commitments_acc_data = &mut commitments_account.data.borrow_mut()[..];
    let mut inserted_tree: CommitmentsAccount<TREE_DEPTH> = CommitmentsAccount::try_from_slice(&commitments_acc_data)?;

    // check if nullifier already exists
    for idx in 0..request.nullifiers.len() {
        if inserted_tree.check_nullifier(&request.nullifiers[idx]) {
            return Err(DarksolError::UtxoAlreadySpent.into());
        }

        inserted_tree.insert_nullifier(request.nullifiers[idx].clone());
    }

    // update nullifiers list
    inserted_tree.serialize(&mut commitments_acc_data)?;

    // transfer token to reciever token account
    transfer_token_out(
        program_id, 
        &[
            funding_account.clone(),
            user_wallet.clone(),
            user_token_account.clone(),
            pda_token_account.clone(),
            token_program.clone(),
        ],
         request.pre_commitments.value
    )?;

    // emit event
    let event = WithdrawEvent {
        receiver: user_token_account.key.to_string(),
        amount: request.pre_commitments.value,
        token_mint_address: fetch_mint_address(user_token_account)?,
    };
    let serialize_event = borsh::to_vec(&event)?;
    sol_log_data(&[b"withdraw_event", &serialize_event]);

    let nullifier_event = NullifierEvent {
        nullifiers: request.nullifiers.clone(),
    };
    let nullifier_serialize_event = borsh::to_vec(&nullifier_event)?;
    sol_log_data(&[b"nullifiers_event", &nullifier_serialize_event]);


    Ok(())
}
