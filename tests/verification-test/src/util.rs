use std::panic;

use darksol::{CommitmentCipherText, DepositRequest, PreCommitments, ShieldCipherText};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use sp1_sdk::{HashableKey, ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use veil_types::{generate_nullifier, keccak, sha256, Arguments, MerkleTreeSparse, PrivateData, PublicData, UTXO};
use rand::Rng;

use crate::METHODS_ELF;

pub fn generate_random_bytes(length: usize) -> Vec<u8> {
    let mut rng = rand::rng();
    (0..length).map(|_| rng.random()).collect()
}

pub fn generate_proof_withdraw(
    tree: MerkleTreeSparse<16>,
    leaves: Vec<Vec<u8>>,
    utxos_in: Vec<UTXO>,
    random_in: Vec<Vec<u8>>,
    amounts_in: Vec<u64>,
    amount_out: u64,
    depositor_spend_key: &Keypair,
    depositor_view_key: &Keypair,
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
            depositor_spend_key.secret().to_bytes().to_vec(),
            depositor_view_key.secret().to_bytes().to_vec(),
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
    let (pk, vk) = client.setup(METHODS_ELF);

    // Generate the proof
    let proof: SP1ProofWithPublicValues = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("failed to generate proof");
    proof.save("bin/methods_proof.bin").unwrap();

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