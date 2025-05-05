use std::panic;

use borsh::BorshSerialize;
use darksol::{CommitmentCipherText, DepositRequest, PreCommitments, ShieldCipherText};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction, transaction::Transaction};
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use spl_associated_token_account::{get_associated_token_address, instruction::create_associated_token_account_idempotent};
use spl_token::instruction::sync_native;
use veil_types::{blind_keys, keccak, sha256, share_key, Arguments, CipherText, CommitmentPlainText, MerkleTreeSparse, PrivateData, PublicData, UTXO};
use aes_gcm::{
    aead::Aead, aes::cipher::generic_array::typenum::U12, Aes256Gcm, Key, KeyInit, Nonce,
};
use rand::Rng;

use crate::METHODS_ELF;

pub fn generate_random_bytes(length: usize) -> Vec<u8> {
    let mut rng = rand::rng();
    (0..length).map(|_| rng.random()).collect()
}

pub async fn create_ata(
    payer: &Keypair,
    client: &RpcClient,
) {
    let payer_pubkey = payer.pubkey();
    let ata = get_associated_token_address(&payer_pubkey, &spl_token::native_mint::ID);

    let amount = 10 * 10_u64.pow(9); /* Wrapped SOL's decimals is 9, hence amount to wrap is 10 SOL */

    // create token account for wrapped sol
    let create_ata_ix = create_associated_token_account_idempotent(
        &payer_pubkey,
        &payer_pubkey,
        &spl_token::native_mint::ID,
        &spl_token::ID,
    );

    let transfer_ix = system_instruction::transfer(&payer_pubkey, &ata, amount);
    let sync_native_ix = sync_native(&spl_token::ID, &ata).unwrap();

    let mut transaction = Transaction::new_with_payer(
        &[create_ata_ix, transfer_ix, sync_native_ix],
        Some(&payer_pubkey),
    );

    transaction.sign(
        &[&payer],
        client.get_latest_blockhash().await.unwrap(),
    );

    let res = client.send_and_confirm_transaction(&transaction).await;

    match res {
        Ok(_) => println!("Initialize transaction successful"),
        Err(err) => println!("Initialize transaction failed: {:?}", err),
    }
}

pub fn generate_proof_transfer(
    tree: MerkleTreeSparse<16>,
    leaves: Vec<Vec<u8>>,
    utxos_in: Vec<UTXO>,
    random_in: Vec<Vec<u8>>,
    amounts_in: Vec<u64>,
    amounts_out: Vec<u64>,
    receiver_viewing_pubkey: Vec<Vec<u8>>,
    receivers_master_pubkey: Vec<Vec<u8>>,
    token_id: &Pubkey,
    sender_viewing_key: &Keypair
) -> (SP1ProofWithPublicValues, Vec<Vec<u8>>, Vec<CommitmentCipherText>, Vec<Vec<u8>>) {
    let sum_in = amounts_in.iter().sum::<u64>();
    let sum_out = amounts_out.iter().sum::<u64>();
    let merkle_proofs = leaves
        .iter()
        .map(|leaf| tree.generate_proof(leaf.clone()))
        .collect::<Vec<_>>();
    let merkle_paths = merkle_proofs
        .iter()
        .map(|proof| proof.path.clone())
        .collect::<Vec<_>>();
    let leaf_indices = merkle_proofs
        .iter()
        .map(|proof| proof.index as u64)
        .collect::<Vec<_>>();

    let nullifiers = utxos_in
        .iter()
        .enumerate()
        .map(|(i, utxo_in)| utxo_in.nullifier(leaf_indices[i]))
        .collect::<Vec<_>>();

    let random_out = generate_random_bytes(32);
    let nonce = generate_random_bytes(32);

    let pubkey = utxos_in[0].spending_public_key();
    let nullifying_key = utxos_in[0].nullifying_key();

    let sender_master_pubkey = sha256(vec![pubkey.as_slice(), nullifying_key.as_slice()]);
    let sender_utxo_pubkey = sha256(vec![sender_master_pubkey.as_slice(), random_out.as_slice()]);

    // let receivers_utxo_pubkey = sha256(vec![receiver_master_pubkey.as_slice(), random_out.as_slice()]);
    let receivers_utxo_pubkey = receivers_master_pubkey.iter()
        .map(|receiver_master_pubkey| {
            sha256(vec![receiver_master_pubkey.as_slice(), random_out.as_slice()])
        })
        .collect::<Vec<_>>();
    if sum_in < sum_out {
        panic!("sum_in < sum_out");
    }

    let mut output_hashes: Vec<Vec<u8>> = vec![];
    let mut new_amounts_out: Vec<u64> = vec![];
    let mut commitment_cipher_texts: Vec<CommitmentCipherText> = vec![];
    if sum_in > sum_out {
        output_hashes.push(
            sha256(vec![sender_utxo_pubkey.as_slice(), token_id.to_bytes().as_slice(), 
                (sum_in - sum_out).to_le_bytes().as_slice()]),
        );
        new_amounts_out.push(sum_in - sum_out);
        commitment_cipher_texts.push(
            encrypt(
                sender_master_pubkey, utxos_in[0].viewing_public_key(), random_out.clone(), sum_in - sum_out, token_id, "sender spare token".to_string(), nonce.clone(), sender_viewing_key)
        );
    }

    amounts_out.iter().enumerate().for_each(|(i, amount_out)| {
        output_hashes.push(
            sha256(vec![receivers_utxo_pubkey[i].as_slice(), token_id.to_bytes().as_slice(), 
                amount_out.to_le_bytes().as_slice()]),
        );
        new_amounts_out.push(*amount_out);
    });
    // amounts_out.push(amount_out);
    let params_hash = keccak(vec![&[100]]);
    let merkle_root = tree.root();
    let signature = utxos_in[0].sign(
        merkle_root.clone(),
        params_hash.clone(),
        nullifiers.clone(),
        output_hashes.clone(),
    );

    let mut utxo_output_keys: Vec<Vec<u8>> = vec![sender_utxo_pubkey];
    receivers_utxo_pubkey.iter().for_each(|receiver_utxo_pubkey| {
        utxo_output_keys.push(receiver_utxo_pubkey.clone());
    });
    
    let public_data = PublicData {
        merkle_root: merkle_root.clone(),
        params_hash,
        nullifiers: nullifiers.clone(),
        output_hashes: output_hashes.clone(),
    };
    let private_data = PrivateData {
        token_id: token_id.to_bytes().to_vec(),
        pubkey,
        signature,
        random_inputs: random_in,
        amount_in: amounts_in,
        merkle_paths,
        merkle_leaf_indices: leaf_indices.clone(),
        nullifying_key,
        utxo_output_keys,
        amount_out: new_amounts_out.clone(),
    };

    let args = Arguments {
        public_data,
        private_data,
        tree_depth: 16u64,
        input_count: utxos_in.len() as u64,
        output_count: new_amounts_out.len() as u64,
    };

    let serialized_args = borsh::to_vec(&args).unwrap();
    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    stdin.write_vec(serialized_args);

    // Setup the program for proving.
    let (pk, _vk) = client.setup(METHODS_ELF);

    // Generate the proof
    let proof: SP1ProofWithPublicValues = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("failed to generate proof");
    proof.save("bin/methods_transfer_proof.bin").unwrap();

    // encrypt the utxo
    receiver_viewing_pubkey.iter().enumerate().for_each(|(i, viewing_pubkey)| {
        let cipher_text = encrypt(
            receivers_master_pubkey[i].clone(), 
            viewing_pubkey.clone(), 
            random_out.clone(), 
            amounts_out[i], 
            token_id, 
            format!("receiver {}", i), 
            nonce.clone(), 
            sender_viewing_key
        );
        commitment_cipher_texts.push(cipher_text);
    });
    (proof, nullifiers, commitment_cipher_texts, output_hashes)
}

pub fn generate_proof_withdraw(
    tree: MerkleTreeSparse<16>,
    leaves: Vec<Vec<u8>>,
    utxos_in: Vec<UTXO>,
    random_in: Vec<Vec<u8>>,
    amounts_in: Vec<u64>,
    amount_out: u64,
    sender_spend_key: &Keypair,
    sender_view_key: &Keypair,
    receiver_spend_key: &Keypair,
    receiver_view_key: &Keypair,
) -> (SP1ProofWithPublicValues, Vec<Vec<u8>>, Option<CommitmentCipherText>, Vec<Vec<u8>>) {
    let sum_in = amounts_in.iter().sum::<u64>();
    let merkle_proofs = leaves
        .iter()
        .map(|leaf| tree.generate_proof(leaf.clone()))
        .collect::<Vec<_>>();
    let merkle_paths = merkle_proofs
        .iter()
        .map(|proof| proof.path.clone())
        .collect::<Vec<_>>();
    let leaf_indices = merkle_proofs
        .iter()
        .map(|proof| proof.index as u64)
        .collect::<Vec<_>>();
    
    let nullifiers = utxos_in
        .iter()
        .enumerate()
        .map(|(i, utxo_in)| utxo_in.nullifier(leaf_indices[i]))
        .collect::<Vec<_>>();
    
    let random_out = generate_random_bytes(32);
    let nonce = generate_random_bytes(32);
    
    let pubkey = utxos_in[0].spending_public_key();
    let nullifying_key = utxos_in[0].nullifying_key();
    
    let mut utxos_out = vec![];
    if sum_in < amount_out {
        panic!("sum_in < amount_out");
    }
    let mut commitment_cipher_text: Option<CommitmentCipherText> = None;
    if sum_in > amount_out {
        let utxo = UTXO::new(
            sender_spend_key.secret().to_bytes().to_vec(),
            sender_view_key.secret().to_bytes().to_vec(),
            spl_token::native_mint::ID.to_bytes().to_vec(),
            random_out.clone(),
            nonce.clone(),
            sum_in - amount_out,
            "test withdraw depositor".to_string(),
        );
        utxos_out.push(
            utxo.clone()
        );
        let cipher_text = utxo.encrypt(receiver_view_key.secret().as_bytes().to_vec());
        commitment_cipher_text = Some(CommitmentCipherText::new(
            cipher_text.blinded_sender_pubkey,
            cipher_text.cipher,
            cipher_text.blinded_receiver_pubkey,
            utxos_out[0].nonce(),
            "".to_string().as_bytes().to_vec(),
        ));
    }

    utxos_out.push(
        UTXO::new(
            receiver_spend_key.secret().to_bytes().to_vec(),
            receiver_view_key.secret().to_bytes().to_vec(),
            spl_token::native_mint::ID.to_bytes().to_vec(),
            random_out.clone(),
            nonce.clone(),
            amount_out,
            "test withdraw receiver".to_string(),
        )
    );
    let output_hashes: Vec<Vec<u8>> = utxos_out.iter().map(|utxo| utxo.utxo_hash()).collect();
    let params_hash = keccak(vec![&[100]]);
    let merkle_root = tree.root();
    let signature = utxos_in[0].sign(
        merkle_root.clone(),
        params_hash.clone(),
        nullifiers.clone(),
        output_hashes.clone(),
    );
    let utxo_output_keys: Vec<Vec<u8>> = utxos_out
        .iter()
        .map(|utxo| utxo.utxo_public_key())
        .collect();
    let public_data = PublicData {
        merkle_root: merkle_root.clone(),
        params_hash,
        nullifiers: nullifiers.clone(),
        output_hashes: output_hashes.clone(),
    };

    let private_data = PrivateData {
        token_id: spl_token::native_mint::ID.to_bytes().to_vec(),
        pubkey,
        signature,
        random_inputs: random_in,
        amount_in: vec![1 * 10_u64.pow(9)],
        merkle_paths,
        merkle_leaf_indices: leaf_indices.clone(),
        nullifying_key,
        utxo_output_keys,
        amount_out: vec![5 * 10_u64.pow(8), 5 * 10_u64.pow(8)],
    };

    let args = Arguments {
        public_data,
        private_data,
        tree_depth: 16u64,
        input_count: utxos_in.len() as u64,
        output_count: utxos_out.len() as u64,
    };

    let serialized_args = borsh::to_vec(&args).unwrap();
    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    stdin.write_vec(serialized_args);

    // Setup the program for proving.
    let (pk, _vk) = client.setup(METHODS_ELF);

    // Generate the proof
    let proof: SP1ProofWithPublicValues = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("failed to generate proof");
    proof.save("bin/methods_withdraw_proof.bin").unwrap();

    // let proof_bytes = proof.bytes();
    // println!("proof length: {}", proof_bytes.len());
    (proof, nullifiers, commitment_cipher_text, output_hashes)
}

pub fn create_deposit_instructions_data_test(
    token_id: &Pubkey,
    amount: u64,
    spending_key: Vec<u8>,
    viewing_key: Vec<u8>,
    deposit_key: Vec<u8>,
    memo: String,
) -> Result<(Vec<u8>, UTXO, Vec<u8>), String> {
    let random = generate_random_bytes(32);
    let utxo = UTXO::new(
        spending_key.clone(),
        viewing_key.clone(),
        token_id.to_bytes().to_vec(),
        random.clone(),
        generate_random_bytes(32),
        amount,
        memo,
    );

    let pre_commitment =
        PreCommitments::new(amount, token_id.to_bytes().to_vec(), utxo.utxo_public_key());
    let deposit_ciphertext = utxo.encrypt_for_deposit(viewing_key.clone(), deposit_key.clone());

    let shield_cipher_text = ShieldCipherText::new(
        deposit_ciphertext.shield_key,
        deposit_ciphertext.cipher,
        utxo.nonce(),
    );

    let request = DepositRequest::new(pre_commitment, shield_cipher_text);
    let instructions_data = match borsh::to_vec(&request) {
        Ok(data) => data,
        Err(err) => return Err(err.to_string()),
    };

    Ok((instructions_data, utxo, random))
}

pub fn encrypt(
    receiver_master_pubkey: Vec<u8>, 
    receiver_viewing_pubkey: Vec<u8>,  
    random_out: Vec<u8>, 
    amount_out: u64,
    token_id: &Pubkey,
    memo: String,
    nonce_bytes: Vec<u8>, 
    sender_viewing_key: &Keypair) -> CommitmentCipherText {
    let sender_viewing_pubkey = sender_viewing_key.pubkey().to_bytes().to_vec();

    let (blinded_sender_pubkey, blinded_receiver_pubkey) = blind_keys(
        sender_viewing_pubkey,
        receiver_viewing_pubkey,
        nonce_bytes.clone(),
    );
    let shared_key = share_key(sender_viewing_key.secret().to_bytes().to_vec(), blinded_receiver_pubkey.clone());

    // encrypt data
    let mut encrypt_key: [u8; 32] = [0; 32];
    encrypt_key.copy_from_slice(&shared_key);
    let key = Key::<Aes256Gcm>::from_slice(&encrypt_key);
    let cipher = Aes256Gcm::new(key);

    let mut random_bytes = [0u8; 12];
    random_bytes.copy_from_slice(&nonce_bytes.clone()[..12]);
    let nonce = Nonce::<U12>::from_slice(&random_bytes);

    let encrypt_data = CommitmentPlainText {
        master_pubkey: receiver_master_pubkey,
        random: random_out,
        amount: amount_out,
        token_id: token_id.to_bytes().to_vec(),
        memo: memo.clone(),
    };
    let mut plain_text = Vec::new();
    encrypt_data.serialize(&mut plain_text).unwrap();

    let ciphertext_bytes = cipher.encrypt(&nonce, plain_text.as_slice()).unwrap();
    let cipher_text = CipherText::new(
        ciphertext_bytes,
        nonce_bytes.clone(),
        blinded_sender_pubkey,
        blinded_receiver_pubkey,
    );
    CommitmentCipherText::new(
        cipher_text.blinded_sender_pubkey,
        cipher_text.cipher,
        cipher_text.blinded_receiver_pubkey,
        nonce_bytes.clone(),
        memo.as_bytes().to_vec().clone(),
    )
}