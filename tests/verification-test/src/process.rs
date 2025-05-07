use base64::engine::general_purpose;
use base64::Engine;
use borsh::BorshDeserialize;
use darksol::merkle::{hash_precommits, CommitmentsAccount};
use darksol::utils::account::get_associated_token_address_and_bump_seed;
use darksol::utils::serialize::BorshDeserializeWithLength;
use darksol::{derive_pda, PreCommitments, SP1Groth16Proof, WithdrawRequest};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::system_program::ID as SYSTEM_PROGRAM_ID;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey;
use solana_sdk::transaction::Transaction;
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signer::Signer, system_instruction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::sync_native;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use veil_types::{generate_nullifier, MerkleTreeSparse, UTXO};

use crate::util::{
    create_deposit_instructions_data_test, generate_proof_withdraw, generate_random_bytes, KeyJson,
};

#[tokio::test]
async fn test_process_instruction() {
    let rpc_client = RpcClient::new_with_commitment(
        String::from("http://127.0.0.1:8899"),
        CommitmentConfig::confirmed(),
    );

    let program_id = pubkey!("BvRRcGvHnbDkJoNTyZbiNVnowBuy9e7XwqFtR4ZQ8ZxY");
    let verification_program_id = pubkey!("4Dhg7uztL1Zbw95VKQ1v45Q7U9xQxoX3jJChSq6jsoKt");

    let payer = solana_sdk::signature::Keypair::new();
    let payer_pubkey = payer.pubkey();

    let depositor_keypair = solana_sdk::signature::Keypair::new();
    let depositor_pubkey = depositor_keypair.pubkey();

    let content = fs::read_to_string("./../../data/output.txt").unwrap();
    let data = general_purpose::STANDARD.decode(content).unwrap();
    let key: KeyJson = KeyJson::try_from_slice(&data).unwrap();

    let spending_key = key.sk.to_vec();
    let viewing_key = key.vk.to_vec();
    let deposit_key = key.dk.to_vec();

    let receiver_keypair = solana_sdk::signature::Keypair::new();
    let receiver_pubkey = receiver_keypair.pubkey();

    let receiver_deposit_key = solana_sdk::signature::Keypair::new();
    let receiver_view_key = solana_sdk::signature::Keypair::new();
    let receiver_spend_key = solana_sdk::signature::Keypair::new();

    let transaction_signature = rpc_client
        .request_airdrop(
            &payer_pubkey,
            200 * solana_sdk::native_token::LAMPORTS_PER_SOL,
        )
        .await
        .unwrap();
    loop {
        if rpc_client
            .confirm_transaction(&transaction_signature)
            .await
            .unwrap()
        {
            break;
        }
    }

    let data_len = 0;
    let rent_exemption_amount = solana_sdk::rent::Rent::default().minimum_balance(data_len);

    let create_acc_ix = system_instruction::create_account(
        &payer.pubkey(),                        // payer
        &depositor_pubkey,                      // new account
        rent_exemption_amount + 10_000_000_000, // rent exemption fee
        data_len as u64,                        // space reseved for new account
        &SYSTEM_PROGRAM_ID,                     //assigned program address
    );

    let mut transaction = Transaction::new_with_payer(&[create_acc_ix], Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &depositor_keypair],
        rpc_client.get_latest_blockhash().await.unwrap(),
    );
    rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .unwrap();

    let create_acc_ix = system_instruction::create_account(
        &payer.pubkey(),                        // payer
        &receiver_pubkey,                       // new account
        rent_exemption_amount + 10_000_000_000, // rent exemption fee
        data_len as u64,                        // space reseved for new account
        &SYSTEM_PROGRAM_ID,                     //assigned program address
    );

    let mut transaction = Transaction::new_with_payer(&[create_acc_ix], Some(&payer.pubkey()));
    transaction.sign(
        &[&payer, &receiver_keypair],
        rpc_client.get_latest_blockhash().await.unwrap(),
    );
    rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .unwrap();

    let depositor_balance = rpc_client.get_balance(&depositor_pubkey).await.unwrap();
    println!("Depositor balance: {}", depositor_balance);

    // initialize

    let mut account_metas: Vec<AccountMeta> = vec![];
    account_metas.push(AccountMeta::new(depositor_pubkey, true));
    let (funding_pda, _bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], &program_id);
    account_metas.push(AccountMeta::new(funding_pda, false));
    let (commitments_pda, _bump_seed) = derive_pda(1, &program_id);
    account_metas.push(AccountMeta::new(commitments_pda, false));
    let (commitments_manager_pda, _bump_seed) =
        Pubkey::find_program_address(&[b"commitments_manager_pda"], &program_id);
    account_metas.push(AccountMeta::new(commitments_manager_pda, false));
    account_metas.push(AccountMeta::new(SYSTEM_PROGRAM_ID, false));

    // let instruction = Instruction {
    //     program_id,
    //     accounts: account_metas,
    //     data: vec![3],
    // };

    // let mut transaction =
    //     Transaction::new_with_payer(&[instruction], Some(&depositor_keypair.pubkey()));

    // transaction.sign(
    //     &[&depositor_keypair],
    //     rpc_client.get_latest_blockhash().await.unwrap(),
    // );
    // rpc_client
    //     .send_and_confirm_transaction(&transaction)
    //     .await
    //     .unwrap();
    println!("run here");

    let ata = get_associated_token_address(&depositor_pubkey, &spl_token::native_mint::ID);

    let amount = 1 * 10_u64.pow(9); /* Wrapped SOL's decimals is 9, hence amount to wrap is 1 SOL */

    // create token account for wrapped sol
    let create_ata_ix = create_associated_token_account_idempotent(
        &depositor_pubkey,
        &depositor_pubkey,
        &spl_token::native_mint::ID,
        &spl_token::ID,
    );

    let transfer_ix = system_instruction::transfer(&depositor_pubkey, &ata, amount);
    let sync_native_ix = sync_native(&spl_token::ID, &ata).unwrap();

    let mut transaction = Transaction::new_with_payer(
        &[create_ata_ix, transfer_ix, sync_native_ix],
        Some(&depositor_pubkey),
    );

    transaction.sign(
        &[&depositor_keypair],
        rpc_client.get_latest_blockhash().await.unwrap(),
    );

    let res = rpc_client.send_and_confirm_transaction(&transaction).await;

    match res {
        Ok(_) => println!("Initialize transaction successful"),
        Err(err) => println!("Initialize transaction failed: {:?}", err),
    }

    let funding_balance = rpc_client.get_balance(&funding_pda).await.unwrap();

    println!("Funding balance: {}", funding_balance);

    // deposit

    let (mut deposit_data, deposit_utxo, deposit_random) =
        match create_deposit_instructions_data_test(
            &spl_token::native_mint::ID,
            amount,
            spending_key.clone(),
            viewing_key.clone(),
            deposit_key.clone(),
            "test deposit".to_string(),
        ) {
            Ok(data) => data,
            Err(err) => {
                println!(
                    "{}",
                    format!("failed to create instruction data: {}", err.to_string())
                );

                return;
            }
        };

    // get current tree number to fetch the correct commitments account info
    let tree_number = 1;

    let ata = get_associated_token_address(&depositor_pubkey, &spl_token::native_mint::ID);

    // get all necessary account meta
    // funding_account
    // user_wallet
    // user_token_account
    // pda_token_account
    // mint_account
    // commitments_account
    // commitments_manager_account
    // token_program
    // system_program
    let mut account_metas: Vec<AccountMeta> = vec![];

    let (funding_pda, bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], &program_id);
    account_metas.push(AccountMeta::new(funding_pda, false));
    account_metas.push(AccountMeta::new(depositor_pubkey, true));
    account_metas.push(AccountMeta::new(ata, false));
    let (funding_ata, ata_bump) = get_associated_token_address_and_bump_seed(
        &funding_pda,
        &spl_token::native_mint::ID,
        &spl_associated_token_account::ID,
        &spl_token::ID,
    );
    let (funding_ata, ata_bump) = Pubkey::find_program_address(&[b"funding_ata"], &program_id);
    println!("bump {:?}", ata_bump);
    account_metas.push(AccountMeta::new(funding_ata, false));
    account_metas.push(AccountMeta::new_readonly(spl_token::native_mint::ID, false));
    let (commitments_pda, _bump_seed) = derive_pda(tree_number, &program_id);
    account_metas.push(AccountMeta::new(commitments_pda, false));
    let (commitments_manager_pda, _bump_seed) =
        Pubkey::find_program_address(&[b"commitments_manager_pda"], &program_id);
    account_metas.push(AccountMeta::new(commitments_manager_pda, false));
    account_metas.push(AccountMeta::new_readonly(spl_token::ID, false));
    account_metas.push(AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false));
    account_metas.push(AccountMeta::new_readonly(
        solana_program::rent::sysvar::ID,
        false,
    ));
    account_metas.push(AccountMeta::new_readonly(
        spl_associated_token_account::ID,
        false,
    ));

    for i in account_metas.iter() {
        println!("Account: {}", i.pubkey);
    }

    // insert variant bytes
    deposit_data.insert(0, 0);
    // Create instruction
    let instruction = Instruction {
        program_id,
        accounts: account_metas,
        data: deposit_data,
    };

    let message = Message::new(&[instruction], Some(&depositor_pubkey));
    let mut transaction = Transaction::new_unsigned(message);

    transaction.sign(
        &[&depositor_keypair],
        rpc_client.get_latest_blockhash().await.unwrap(),
    );

    let res = rpc_client.send_and_confirm_transaction(&transaction).await;

    match res {
        Ok(_) => println!("Deposit transaction successful"),
        Err(err) => panic!("Deposit transaction failed: {:?}", err),
    };

    let commitments_account: CommitmentsAccount<31>;
    match rpc_client.get_account(&commitments_pda).await {
        Ok(account) => {
            let account_data = account.data;
            commitments_account =
                CommitmentsAccount::try_from_slice_with_length(&account_data).unwrap();
            assert!(commitments_account.next_leaf_index == 1);
        }
        Err(e) => panic!("Failed to get account data: {}", e),
    };

    let mut tree = MerkleTreeSparse::<16>::new(1);
    let pre_commitment = PreCommitments::new(
        amount,
        spl_token::native_mint::ID.to_bytes().to_vec(),
        deposit_utxo.utxo_public_key(),
    );
    let inserted_leaf = hash_precommits(pre_commitment);

    tree.insert(vec![inserted_leaf.clone()]);
    assert_eq!(tree.root(), commitments_account.root());

    // // create receiver token account
    // let receiver_token_addr =
    //     get_associated_token_address(&receiver_pubkey, &spl_token::native_mint::ID);

    // // generate proof
    // use std::time::Instant;
    // let now = Instant::now();
    // let (proof, nullifiers, ciphertext, utxo_hashes) = generate_proof_withdraw(
    //     tree.clone(),
    //     vec![inserted_leaf.clone()],
    //     vec![deposit_utxo.clone()],
    //     vec![deposit_random.clone()],
    //     vec![1 * 10_u64.pow(9)],
    //     5 * 10_u64.pow(8),
    //     &depositor_spend_key,
    //     &depositor_view_key,
    //     &receiver_spend_key,
    //     &receiver_view_key,
    // );
    // println!("Time taken to generate proof: {:?}", now.elapsed());
    // let ciphertext = ciphertext.unwrap();
    // // let proof_bytes = vec![];

    // // let utxo_out = UTXO::new(
    // //     depositor_spend_key.secret().to_bytes().to_vec(),
    // //     depositor_view_key.secret().to_bytes().to_vec(),
    // //     spl_token::native_mint::ID.to_bytes().to_vec(),
    // //     generate_random_bytes(32),
    // //     generate_random_bytes(32),
    // //     5 * 10_u64.pow(8),
    // //     "test withdraw depositor".to_string(),
    // // );
    // // let nullifer = generate_nullifier(receiver_view_key.secret().as_bytes().to_vec(), 0);

    // let mut withdraw_request = WithdrawRequest::new(
    //     proof.bytes().to_vec(),
    //     tree.root(),
    //     tree_number,
    //     5 * 10_u64.pow(8),
    //     spl_token::native_mint::ID.to_bytes().to_vec(),
    //     vec![ciphertext],
    // );
    // nullifiers.iter().for_each(|nullifier| {
    //     withdraw_request.push_nullifiers(nullifier.clone());
    // });
    // utxo_hashes.iter().for_each(|utxo_hash| {
    //     withdraw_request.push_encrypted_commitment(utxo_hash.clone());
    // });

    // let mut serialized_data = match borsh::to_vec(&withdraw_request) {
    //     Ok(data) => data,
    //     Err(err) => panic!("{}", err.to_string()),
    // };

    // // get all necessary account meta
    // // funding account
    // // spent commitments account
    // // user wallet
    // // user token account
    // // funding token account
    // // token program
    // //
    // // current commitment account
    // // commitments manager account

    // let mut account_metas: Vec<AccountMeta> = vec![];

    // let (funding_pda, _bump_seed) = Pubkey::find_program_address(&[b"funding_pda"], &program_id);
    // account_metas.push(AccountMeta::new(funding_pda, false));
    // let (spent_commitments_pda, _bump_seed) = derive_pda(tree_number, &program_id);
    // account_metas.push(AccountMeta::new(spent_commitments_pda, false));
    // let (commitments_manager_pda, _bump_seed) =
    //     Pubkey::find_program_address(&[b"commitments_manager_pda"], &program_id);
    // account_metas.push(AccountMeta::new(commitments_manager_pda, false));
    // account_metas.push(AccountMeta::new(receiver_pubkey, false));
    // account_metas.push(AccountMeta::new(receiver_token_addr, false));
    // let (funding_ata, ata_bump) = Pubkey::find_program_address(&[b"funding_ata"], &program_id);
    // account_metas.push(AccountMeta::new(funding_ata, false));
    // account_metas.push(AccountMeta::new_readonly(spl_token::ID, false));
    // account_metas.push(AccountMeta::new(verification_program_id, false));
    // let (current_commitments_pda, _bump_seed) = derive_pda(tree_number, &program_id);
    // account_metas.push(AccountMeta::new_readonly(current_commitments_pda, false));
    // // insert variant bytes
    // serialized_data.insert(0, 2);
    // println!("data length: {}", serialized_data.len());
    // // Create instruction
    // let instruction = Instruction {
    //     program_id,
    //     accounts: account_metas,
    //     data: serialized_data,
    // };

    // let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(5_000_000u32);
    // let message = Message::new(&[cu_ix, instruction], Some(&&receiver_pubkey));

    // let mut transaction = Transaction::new_unsigned(message);

    // transaction.sign(
    //     &[&receiver_keypair],
    //     rpc_client.get_latest_blockhash().await.unwrap(),
    // );

    // let res = rpc_client.send_and_confirm_transaction(&transaction).await;

    // match res {
    //     Ok(_) => println!("Withdraw transaction successful"),
    //     Err(err) => {
    //         println!("proof: {:?}", proof.bytes());
    //         println!("public values: {:?}", proof.public_values.to_vec());
    //         let proof = SP1Groth16Proof {
    //             proof: proof.bytes(),
    //             sp1_public_inputs: proof.public_values.to_vec(),
    //         };
    //         let bytes_proof = borsh::to_vec(&proof).unwrap();
    //         println!("proof: {:?}", bytes_proof);
    //         panic!("Withdraw transaction failed: {:?}", err)
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::BorshSerialize;
    use darksol::SP1Groth16Proof;
    use veil_types::PublicValue;

    #[test]
    fn test_borsh() {
        let proof = SP1Groth16Proof::try_from_slice(&[
            4, 1, 0, 0, 17, 182, 160, 157, 10, 186, 9, 9, 152, 102, 31, 155, 238, 8, 195, 167, 114,
            232, 190, 200, 235, 147, 124, 223, 71, 144, 54, 133, 12, 221, 241, 80, 107, 55, 1, 204,
            14, 16, 103, 146, 33, 15, 152, 106, 38, 229, 205, 113, 123, 123, 51, 237, 130, 96, 218,
            106, 8, 87, 224, 50, 152, 236, 219, 74, 22, 72, 200, 60, 47, 71, 233, 223, 228, 126,
            98, 62, 122, 84, 98, 57, 249, 249, 111, 137, 76, 218, 158, 233, 243, 45, 161, 29, 37,
            145, 124, 72, 86, 123, 204, 117, 39, 250, 82, 150, 191, 5, 137, 116, 122, 229, 75, 227,
            139, 231, 19, 208, 101, 83, 81, 181, 48, 49, 50, 147, 31, 79, 166, 184, 144, 86, 218,
            234, 6, 93, 246, 2, 55, 146, 177, 238, 134, 66, 120, 59, 89, 67, 13, 10, 113, 149, 9,
            140, 101, 195, 127, 16, 212, 61, 116, 198, 80, 59, 79, 161, 41, 139, 74, 3, 51, 29,
            176, 63, 65, 52, 147, 140, 94, 125, 95, 140, 143, 13, 53, 205, 168, 236, 146, 100, 135,
            81, 52, 247, 57, 16, 95, 237, 37, 223, 21, 39, 244, 229, 36, 186, 177, 63, 171, 228,
            254, 171, 90, 80, 10, 189, 73, 251, 178, 38, 61, 224, 207, 58, 190, 67, 132, 77, 167,
            123, 37, 90, 23, 9, 0, 61, 130, 183, 200, 72, 73, 79, 60, 184, 15, 30, 31, 67, 211,
            156, 10, 150, 193, 26, 242, 33, 8, 249, 224, 196, 113, 158, 152, 0, 0, 0, 32, 0, 0, 0,
            224, 230, 187, 65, 40, 252, 149, 61, 79, 113, 129, 248, 246, 135, 31, 29, 199, 122,
            157, 183, 135, 119, 185, 20, 74, 176, 96, 93, 96, 136, 61, 208, 1, 0, 0, 0, 32, 0, 0,
            0, 116, 141, 164, 228, 193, 138, 134, 207, 248, 123, 144, 214, 105, 159, 127, 183, 119,
            4, 166, 153, 155, 146, 199, 57, 187, 149, 43, 117, 101, 221, 107, 72, 2, 0, 0, 0, 32,
            0, 0, 0, 111, 126, 93, 43, 120, 19, 150, 57, 2, 235, 142, 12, 175, 13, 177, 135, 221,
            79, 120, 211, 200, 145, 212, 171, 191, 66, 191, 61, 243, 72, 83, 227, 32, 0, 0, 0, 239,
            166, 186, 98, 220, 180, 81, 241, 69, 7, 51, 13, 49, 177, 47, 100, 203, 21, 70, 48, 109,
            33, 230, 147, 2, 42, 140, 39, 157, 78, 2, 9,
        ])
        .unwrap();
        let public_value = PublicValue::try_from_slice(&[
            32, 0, 0, 0, 224, 230, 187, 65, 40, 252, 149, 61, 79, 113, 129, 248, 246, 135, 31, 29,
            199, 122, 157, 183, 135, 119, 185, 20, 74, 176, 96, 93, 96, 136, 61, 208, 1, 0, 0, 0,
            32, 0, 0, 0, 116, 50, 114, 51, 241, 20, 132, 75, 148, 213, 166, 167, 89, 76, 108, 134,
            48, 46, 103, 56, 108, 108, 202, 109, 99, 87, 48, 68, 136, 22, 72, 178, 2, 0, 0, 0, 32,
            0, 0, 0, 111, 126, 93, 43, 120, 19, 150, 57, 2, 235, 142, 12, 175, 13, 177, 135, 221,
            79, 120, 211, 200, 145, 212, 171, 191, 66, 191, 61, 243, 72, 83, 227, 32, 0, 0, 0, 239,
            166, 186, 98, 220, 180, 81, 241, 69, 7, 51, 13, 49, 177, 47, 100, 203, 21, 70, 48, 109,
            33, 230, 147, 2, 42, 140, 39, 157, 78, 2, 9,
        ])
        .unwrap();
        let proof_bytes = borsh::to_vec(&proof).unwrap();
        let sp1_public_value = PublicValue::try_from_slice(&proof.sp1_public_inputs).unwrap();
        assert_eq!(public_value.nullifiers, sp1_public_value.nullifiers);
        assert_eq!(public_value.root, sp1_public_value.root);
        assert_eq!(public_value.output_hashes, sp1_public_value.output_hashes);
        println!("proof: {:?}", proof_bytes);
    }
}
