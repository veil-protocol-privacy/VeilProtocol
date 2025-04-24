use std::ops::{AddAssign, SubAssign};

use crate::merkle::CommitmentsAccount;
use crate::state::initialize_commitments_manager;
use crate::utils::account::{
    create_pda_account_from_pda_account, get_associated_token_address_and_bump_seed,
};
use crate::utils::serialize::{BorshDeserializeWithLength, BorshSerializeWithLength};
use crate::{
    derive_pda, DepositEvent, DepositRequest, NullifierEvent, SP1Groth16Proof, TransactionEvent,
    TransferRequest, WithdrawRequest,
};
use crate::{
    error::DarksolError,
    merkle::hash_precommits,
    state::{initialize_commitments_account, CommitmentsManagerAccount},
    TREE_DEPTH,
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::instruction::Instruction;
use solana_program::log::sol_log_data;
use solana_program::msg;
use solana_program::program::invoke;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use veil_types::PublicData;

use solana_program::system_instruction;
use solana_program::sysvar::{rent::Rent, Sysvar};

use spl_associated_token_account::get_associated_token_address;
use spl_associated_token_account::instruction::create_associated_token_account;
use spl_token::instruction::{initialize_account, transfer as spl_transfer};
use spl_token::{solana_program::program_pack::Pack, state::Account};

// transfer_token_in deposit user fund into contract owned account.
// Create a new token account for the deposit account if it's not initialized yet
// TODO: handle both native and spl token transfer
fn transfer_token_in(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let accounts_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();

    let funding_account = next_account_info(accounts_iter)?;
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let associated_token_account = next_account_info(accounts_iter)?; // PDA token account
    let mint_account = next_account_info(accounts_iter)?; // SPL Token Mint
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program
    let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts
    let rent_sysvar = next_account_info(accounts_iter)?; // Rent Sysvar
    let ata_program = next_account_info(accounts_iter)?; // Associated Token Program

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], program_id);
    if funding_account.key != &funding_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    let (funding_ata, ata_bump) = get_associated_token_address_and_bump_seed(
        &*funding_account.key,
        &mint_account.key,
        ata_program.key,
        token_program.key,
    );

    let (funding_ata, ata_bump) = Pubkey::find_program_address(&[b"funding_ata"], program_id);

    msg!("funding_ata: {:?}", ata_bump);

    let ata_seed: &[&[u8]] = &[
        b"funding_ata",
        // &funding_account.key.to_bytes(),
        // &token_program.key.to_bytes(),
        // &mint_account.key.to_bytes(),
        &[ata_bump],
    ];

    if associated_token_account.key != &funding_ata {
        return Err(ProgramError::InvalidSeeds);
    }

    // TODO: apply deposit fee

    // Check if PDA's token account is already initialized
    if associated_token_account.data_is_empty() {
        // PDA's token account is not initialized → Create it
        // create_pda_account_from_pda_account(
        //     funding_account,
        //     spl_token::state::Account::LEN,
        //     token_program.key,
        //     system_program,
        //     associated_token_account,
        //     ata_seed,
        // )?;

        let rent: &Rent = &Rent::get()?;

        let required_lamports = rent.minimum_balance(spl_token::state::Account::LEN);

        msg!("Allocating PDA");
        invoke_signed(
            &system_instruction::allocate(
                associated_token_account.key,
                spl_token::state::Account::LEN.try_into().unwrap(),
            ),
            &[associated_token_account.clone(), system_program.clone()],
            &[ata_seed],
        )?;

        msg!("Assigning PDA");
        invoke_signed(
            &system_instruction::assign(associated_token_account.key, token_program.key),
            &[associated_token_account.clone(), system_program.clone()],
            &[ata_seed],
        )?;

        msg!("Funding PDA");
        if required_lamports > 0 {
            funding_account
                .lamports
                .borrow_mut()
                .sub_assign(required_lamports);
            associated_token_account
                .lamports
                .borrow_mut()
                .add_assign(required_lamports);
        }

        msg!("Initializing token account");
        invoke_signed(
            // TODO: emit error
            &initialize_account(
                token_program.key,
                associated_token_account.key,
                mint_account.key,
                &funding_account.key, // PDA is the owner of this token account
            )?,
            &[
                associated_token_account.clone(),
                mint_account.clone(),
                funding_account.clone(),
                rent_sysvar.clone(),
                token_program.clone(),
            ],
            &[&[b"funding_pda", &[bump_seed]]], // PDA signs
        )?;
    }
    msg!("created funding ata");

    // transfer token to contract owned token account
    invoke(
        // TODO: emit error
        &spl_transfer(
            token_program.key,
            user_token_account.key,
            associated_token_account.key,
            user_wallet.key, // User must sign as authority
            &[],
            amount,
        )?,
        &[
            user_token_account.clone(),
            associated_token_account.clone(),
            user_wallet.clone(),
            token_program.clone(),
        ],
    )?;
    msg!("transfered to funding ata");

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
    if funding_account.key != &funding_pda
        || pda_token_account.owner != &funding_pda
        || user_token_account.owner != user_wallet.key
    {
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
    let user_ata_account = next_account_info(accounts_iter)?; // User's SPL token account
    let funding_ata_account = next_account_info(accounts_iter)?; // PDA token account
    let mint_account: &AccountInfo<'_> = next_account_info(accounts_iter)?; // SPL Token Mint
    let commitments_account = next_account_info(accounts_iter)?; // current commitments account
    let commitments_manager_account = next_account_info(accounts_iter)?;
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program
    let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts
    let rent_sysvar = next_account_info(accounts_iter)?; // Rent Sysvar
    let ata_program = next_account_info(accounts_iter)?; // Associated Token Program

    if commitments_account.owner != program_id || commitments_manager_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // fetch the current tree number
    let data = commitments_manager_account.data.borrow_mut();

    msg!("data length: {:?}", data.len());
    msg!("data: {:?}", data);

    // deserialize the data
    let manager_data: CommitmentsManagerAccount =
        CommitmentsManagerAccount::try_from_slice_with_length(&data)?;

    msg!("manager_data: {:?}", manager_data);

    // Derive the PDA for the current commitments account
    let (account_pda, _bump_seed) = derive_pda(manager_data.incremental_tree_number, program_id);

    msg!("tree number: {:?}", manager_data.incremental_tree_number);
    // Ensure the provided new_account is the correct PDA
    if commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    msg!("transfer token in");

    // transfer token to contract owned account
    transfer_token_in(
        program_id,
        &[
            funding_account.clone(),
            user_wallet.clone(),
            user_ata_account.clone(),
            funding_ata_account.clone(),
            mint_account.clone(),
            token_program.clone(),
            system_program.clone(),
            rent_sysvar.clone(),
            ata_program.clone(),
        ],
        request.pre_commitments.value,
    )?;

    msg!("transfered token in");

    let inserted_leaf = hash_precommits(request.pre_commitments.clone());

    // fetch current tree number
    let mut commitments_data = &mut commitments_account.data.borrow_mut()[..];

    // deserialize the data
    let mut current_tree = CommitmentsAccount::try_from_slice_with_length(&commitments_data)?;
    let mut current_tree_number: u64 = manager_data.incremental_tree_number;
    let start_position: u64 = 0;

    msg!("current tree number: {:?}", current_tree_number);

    // create new commitments account if insert leaf exceeds max tree depth
    // user should check if the inserted leafs exceeds max tree depth to
    // add new commitments account to the instruction
    if current_tree.exceed_tree_depth(1) {
        msg!("exceed_tree_depth");
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
                resp.commitments_data
                    .serialize_with_length(&mut new_commitments_data)?
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }
    } else {
        // insert leaf into tree
        msg!("not exceed_tree_depth");

        msg!("commitments_data length: {:?}", commitments_data.len());
        let result = current_tree.insert_commitments(&mut vec![inserted_leaf.clone()]);
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data
                    .serialize_with_length(&mut commitments_data)?;

                msg!("commitments_data length: {:?}", commitments_data.len());
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

    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let spent_commitments_account = next_account_info(accounts_iter)?; // commitments account contains spent UTXO
    let current_commitments_account = next_account_info(accounts_iter)?; // current commitments account
    let commitments_manager_account = next_account_info(accounts_iter)?;
    // let verification_account = next_account_info(accounts_iter)?; // verification account

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if spent_commitments_account.owner != program_id
        || commitments_manager_account.owner != program_id
    {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive the PDA for the commitments account
    let (account_pda, _bump_seed) = derive_pda(request.metadata.tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if spent_commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // fetch the current tree number
    let data = commitments_manager_account.data.borrow_mut();
    // deserialize the data
    let manager_data: CommitmentsManagerAccount =
        CommitmentsManagerAccount::try_from_slice_with_length(&data)?;
    let mut current_tree_number = manager_data.incremental_tree_number;

    // Derive the PDA for the commitments account
    let (pda, _bump_seed) = derive_pda(current_tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if current_commitments_account.key != &pda {
        return Err(ProgramError::InvalidSeeds);
    }

    let mut spent_commitments_acc_data = &mut spent_commitments_account.data.borrow_mut()[..];
    let mut spent_tree: CommitmentsAccount<TREE_DEPTH> =
        CommitmentsAccount::try_from_slice_with_length(&spent_commitments_acc_data)?;

    let mut current_commitments_acc_data =
        if current_commitments_account.key == spent_commitments_account.key {
            &mut spent_commitments_acc_data
        } else {
            &mut current_commitments_account.data.borrow_mut()[..]
        };
    let mut inserted_tree: CommitmentsAccount<TREE_DEPTH> =
        CommitmentsAccount::try_from_slice_with_length(&current_commitments_acc_data)?;

    // Deserialize the SP1Groth16Proof from the instruction data.
    let groth16_proof = SP1Groth16Proof::try_from_slice(&request.proof)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let public_values = groth16_proof.sp1_public_inputs.as_slice();
    let public_data = PublicData::try_from_slice(public_values)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    if public_data.merkle_root.ne(&request.merkle_root) {
        return Err(DarksolError::MerkleRootNotMatch.into());
    }
    if public_data.nullifiers.ne(&request.nullifiers) {
        return Err(DarksolError::NullifiersNotMatch.into());
    }

    // Create an instruction to invoke the verification program.
    // let instruction = Instruction::new_with_borsh(
    //     *verification_account.key,
    //     &groth16_proof,
    //     vec![],
    // );
    // invoke(&instruction, accounts)?;

    // check if merkle root is valid
    msg!("request merkle root: {:?}", request.merkle_root);

    if !spent_tree.has_root(request.merkle_root) {
        return Err(DarksolError::InvalidMerkelRoot.into());
    }

    // ------------------- verify logic end here ------------------------ //

    // check if nullifiers already exists if not added to the list
    for idx in 0..request.nullifiers.len() {
        if spent_tree.check_nullifier(&request.nullifiers[idx]) {
            return Err(DarksolError::UtxoAlreadySpent.into());
        }

        spent_tree.insert_nullifier(request.nullifiers[idx].clone());
    }

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
        // insert leaf into tree
        let result = inserted_tree.insert_commitments(&mut request.encrypted_commitments.clone());
        match result {
            Ok(resp) => {
                // update tree
                resp.commitments_data
                    .serialize(&mut current_commitments_acc_data)?;

                start_position = resp.commitments_data.next_leaf_index as u64;
            }
            Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
        }
    }

    // update nullifiers list
    spent_tree.serialize(&mut spent_commitments_acc_data)?;

    // emit event
    let event = TransactionEvent {
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
    let spent_commitments_account = next_account_info(accounts_iter)?; // commitments account for the request tree number
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program

    if spent_commitments_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive the PDA for the commitments account
    let (account_pda, _bump_seed) = derive_pda(request.metadata.tree_number, program_id);
    // Ensure the provided new_account is the correct PDA
    if spent_commitments_account.key != &account_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut commitments_acc_data = &mut spent_commitments_account.data.borrow_mut()[..];
    let mut spent_tree: CommitmentsAccount<TREE_DEPTH> =
        CommitmentsAccount::try_from_slice(&commitments_acc_data)?;

    // Deserialize the SP1Groth16Proof from the instruction data.
    let groth16_proof = SP1Groth16Proof::try_from_slice(&request.proof)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let public_values = groth16_proof.sp1_public_inputs.as_slice();
    let public_data = PublicData::try_from_slice(public_values)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    if public_data.merkle_root.eq(&request.merkle_root) {
        return Err(DarksolError::MerkleRootNotMatch.into());
    }
    if public_data.nullifiers.eq(&request.nullifiers) {
        return Err(DarksolError::NullifiersNotMatch.into());
    }
    
    
    let precommitments_hash = request.pre_commitments.hash();
    if public_data.output_hashes.last().is_some() {
        let last_hash = public_data.output_hashes.last().unwrap();
        if !last_hash.eq(&precommitments_hash) {
        return Err(DarksolError::PreCommitmentHashNotMatch.into());
       }
    }

    // check if merkle root is valid
    if !spent_tree.has_root(request.merkle_root) {
        return Err(DarksolError::InvalidMerkelRoot.into());
    }

    // ------------------ verify logic end ---------------------- //

    // check if nullifier already exists
    for idx in 0..request.nullifiers.len() {
        if spent_tree.check_nullifier(&request.nullifiers[idx]) {
            return Err(DarksolError::UtxoAlreadySpent.into());
        }

        spent_tree.insert_nullifier(request.nullifiers[idx].clone());
    }

    let mut start_position: u64 = spent_tree.next_leaf_index as u64;
    let mut tree_number: u64 = request.metadata.tree_number;

    if !request.encrypted_commitments.is_empty() {
        let current_commitment_account = next_account_info(accounts_iter)?; // current tree
        let commitments_manager_account = next_account_info(accounts_iter)?;

        // fetch the current tree number
        let data = commitments_manager_account.data.borrow_mut();
        // deserialize the data
        let manager_data: CommitmentsManagerAccount =
            CommitmentsManagerAccount::try_from_slice(&data)?;
        let current_tree_number = manager_data.incremental_tree_number;

        let mut commitments_acc_data = &mut current_commitment_account.data.borrow_mut()[..];
        let mut inserted_tree: CommitmentsAccount<TREE_DEPTH> =
            CommitmentsAccount::try_from_slice(&commitments_acc_data)?;

        // Derive the PDA for the commitments account
        let (account_pda, _bump_seed) = derive_pda(current_tree_number, program_id);
        // Ensure the provided new_account is the correct PDA
        if current_commitment_account.key != &account_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        // update merkle tree
        // create new commitments account if insert leaf exceeds max tree depth
        // user should check if the inserted leafs exceeds max tree depth to
        // add new commitments account, funding account and system program to the instruction
        if inserted_tree.exceed_tree_depth(1) {
            let funding_account = next_account_info(accounts_iter)?;
            let new_commitments_account = next_account_info(accounts_iter)?; // new commitments account
            let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts

            // derive a new commitments account and update the commitments account
            let (new_pda, _bump_seed) = derive_pda(current_tree_number + 1, program_id);

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

            let mut new_commitments_data = &mut new_commitments_account.data.borrow_mut()[..];

            // insert leaf into tree
            let result =
                inserted_tree.insert_commitments(&mut request.encrypted_commitments.clone());
            match result {
                Ok(resp) => {
                    // update tree
                    resp.commitments_data.serialize(&mut new_commitments_data)?;

                    tree_number = current_tree_number + 1;
                    start_position = resp.commitments_data.next_leaf_index as u64;
                }
                Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
            }
        } else {
            // insert leaf into tree
            let result =
                inserted_tree.insert_commitments(&mut request.encrypted_commitments.clone());
            match result {
                Ok(resp) => {
                    // update tree
                    resp.commitments_data.serialize(&mut commitments_acc_data)?;

                    tree_number = current_tree_number;
                    start_position = resp.commitments_data.next_leaf_index as u64;
                }
                Err(_err) => return Err(DarksolError::FailedInsertCommitmentHash.into()),
            }
        }
    }

    // update nullifiers list
    spent_tree.serialize(&mut commitments_acc_data)?;

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
        request.pre_commitments.value,
    )?;

    // emit event
    let event = TransactionEvent {
        start_position,
        tree_number,
        commitments: request.encrypted_commitments.clone(),
        commitment_cipher_text: request.commitment_cipher_texts.clone(),
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

pub fn process_initialize_account(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("Hello");
    initialize_commitments_manager(program_id, accounts)?;

    Ok(())
}
