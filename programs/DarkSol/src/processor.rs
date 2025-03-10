use crate::{u256_to_bytes, ZERO_VALUE};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_poseidon::hashv;
use solana_poseidon::{Endianness, Parameters, PoseidonHash};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::{
    instruction::{initialize_account, transfer as spl_transfer},
    state::Account as TokenAccount,
};
use std::clone;
use std::collections::HashMap;
use std::fmt;

// process_deposit_fund deposit user fund into contract owned account
// this will be added into unshield transactions list. Create a new token account
// for the deposit account if it's not initialized yet
pub fn process_deposit_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: u64,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let accounts_iter = &mut accounts.iter();
    let user_wallet = next_account_info(accounts_iter)?; // User's SOL wallet (payer)
    let user_token_account = next_account_info(accounts_iter)?; // User's SPL token account
    let pda_token_account = next_account_info(accounts_iter)?; // PDA token account
    let mint_account = next_account_info(accounts_iter)?; // SPL Token Mint
    let pda_account = next_account_info(accounts_iter)?; // PDA acting as authority
    let token_program = next_account_info(accounts_iter)?; // SPL Token Program
    let system_program = next_account_info(accounts_iter)?; // System Program for creating accounts

    // Ensure the user_wallet signed the transaction
    if !user_wallet.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Derive PDA funding account to pay for the new account
    // TODO: change the seeds
    let (funding_pda, _funding_bump_seed) =
        Pubkey::find_program_address(&[b"funding_pda"], program_id);
    if payer_account.key != &funding_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Derive PDA from program ID
    // TODO: change seeds
    let (expected_pda, bump_seed) = Pubkey::find_program_address(&[b"deposit_account"], program_id);

    if pda_account.key != &expected_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Check if PDA's token account is already initialized
    if pda_token_account.data_is_empty() {
        // PDA's token account is not initialized → Create it

        let rent = &Rent::get()?;
        let required_lamports = rent.minimum_balance(TokenAccount::LEN);

        invoke(
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
        )?;

        invoke_signed(
            // TODO: emit error
            &initialize_account(
                token_program.key,
                pda_token_account.key,
                mint_account.key,
                pda_account.key, // PDA is the owner of this token account
            ).unwrap(),
            &[
                pda_token_account.clone(),
                mint_account.clone(),
                pda_account.clone(),
                token_program.clone(),
                rent_sysvar.clone(),
            ],
            &[&[b"deposit_account", &[bump_seed]]], // PDA signs
        )?;
    }

    invoke(
        // TODO: emit error
        &spl_transfer(
            token_program.key,
            user_token_account.key,
            pda_token_account.key,
            user_wallet.key, // User must sign as authority
            &[],
            amount,
        ).unwrap(),
        &[
            user_token_account.clone(),
            pda_token_account.clone(),
            user_wallet.clone(),
            token_program.clone(),
        ],
    )?;

    // TODO: add an unshielded entry to the list

    Ok(())
}

// process_shielded_asset take the unshieled asset in the list
// add an encrypted commitments to the merkel tree represents 
// an unspent entry transaction output. Only the owner of that
// UTXO can decrypt it.
pub fn process_hide_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    encrypted_commitments: Vec<u8>,
) -> ProgramResult {
    // TODO: update UnshieldedAsset data

    // TODO: insert UTXO to current merkel tree account
    Ok(())
}

// process_transfer_asset takes the zkp for ownership of the UTXO
// in the merkel tree, create a nullifier for that UTXO to marked it
// as spent and inserts new encrypted UTXO commitments to the merkle tree
pub fn process_transfer_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    proofs: Vec<u8>,
) -> ProgramResult {
    // TODO: verify proof

    // TODO: create nullifier adn check if nullifier already exists

    // TODO: update merkle tree
    Ok(())
}

// process_withdraw_asset verifies the zkp for ownership of the UTXO
// transfer the token from contract vault token account to receiver 
// token account
pub fn process_withdraw_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    proofs: Vec<u8>,
) -> ProgramResult {
    // TODO: verify proof

    // TODO: create nullifier adn check if nullifier already exists

    // TODO: transfer token to reciever token account
    Ok(())
}