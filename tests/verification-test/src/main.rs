
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, compute_budget::ComputeBudgetInstruction, instruction::{AccountMeta, Instruction}, native_token::LAMPORTS_PER_SOL, program_pack::Pack, pubkey, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction::create_account, transaction::Transaction
};

use verification::{process_instruction};
use veil_types::{PublicData, MerkleTreeSparse, SP1Groth16Proof, PrivateData, CipherText, UTXO, Arguments, sha256, keccak};

use borsh::{BorshDeserialize, BorshSerialize};

pub mod process;
pub mod util;
/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const METHODS_ELF: &[u8] =  include_bytes!("../bin/methods");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rpc_client = RpcClient::new_with_commitment(
        String::from("http://127.0.0.1:8899"),
        CommitmentConfig::confirmed(),
    );
    let verification_program_id = pubkey!("DmjXKQCZR8ayWBJBhMwHtdXbmbMNj45fM4kqJVwMJUHd");
    let mock_invoke_program_id = pubkey!("8xtWk9heYnMtkTh8yEyghVtrvBaYs7Wp44KRG1ewrZNT");

    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // let proof = prove();
    let proof = SP1ProofWithPublicValues::load("bin/methods_proof.bin").expect("loading proof failed");

    let proof = SP1Groth16Proof {
        proof: proof.bytes().to_vec(),
        sp1_public_inputs: proof.public_values.to_vec(),
    };

    let payer = Keypair::new();

    let transaction_signature = rpc_client
        .request_airdrop(&payer.pubkey(), 5 * LAMPORTS_PER_SOL)
        .await?;
    loop {
        if rpc_client.confirm_transaction(&transaction_signature).await? {
            break;
        }
    }


    let instruction = Instruction::new_with_borsh(
        mock_invoke_program_id,
        &proof,
        vec![AccountMeta::new(verification_program_id, false)],
    );

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(500_000u32);

    let mut transaction = Transaction::new_with_payer(&[cu_ix, instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], rpc_client.get_latest_blockhash().await?);
    match rpc_client.send_and_confirm_transaction(&transaction).await {
        Ok(signature) => println!("Transaction Signature: {}", signature),
        Err(err) => eprintln!("Error sending transaction: {}", err),
    }
    Ok(())
}
// merkle_root: [73, 150, 96, 27, 164, 245, 65, 244, 154, 243, 248, 228, 15, 115, 13, 86, 69, 143, 212, 163, 28, 158, 22, 6, 221, 64, 178, 99, 148, 135, 242, 147]
// nullifiers: [[5, 151, 163, 15, 67, 207, 227, 86, 41, 82, 143, 243, 143, 225, 74, 137, 143, 38, 6, 239, 90, 10, 24, 204, 163, 227, 72, 225, 160, 207, 27, 59], [171, 151, 212, 17, 115, 144, 195, 29, 78, 100, 239, 188, 29, 33, 222, 246, 191, 58, 68, 241, 192, 146, 32, 154, 183, 27, 161, 246, 166, 135, 66, 185], [93, 221, 58, 1, 25, 32, 103, 128, 236, 251, 199, 206, 68, 201, 146, 223, 229, 93, 188, 250, 110, 172, 118, 205, 115, 111, 106, 30, 157, 169, 224, 56]]

fn prove() -> SP1ProofWithPublicValues {
    // Setup the prover client.
    let client = ProverClient::from_env();

    let spending_key_1 = vec![115, 174, 166, 214, 50, 27, 235, 19, 181, 112, 191, 33, 121, 246, 98, 67, 85, 126, 234, 211, 159, 202, 185, 134, 53, 109, 41, 45, 73, 218, 31, 218];
    let spending_key_2 = vec![30, 16, 96, 72, 220, 113, 73, 111, 15, 147, 214, 92, 171, 174, 4, 112, 38, 142, 49, 205, 238, 205, 77, 214, 124, 210, 122, 218, 148, 61, 75, 195];
    let viewing_key_1 = vec![93, 67, 166, 137, 242, 195, 179, 2, 150, 65, 198, 92, 80, 8, 0, 92, 135, 48, 79, 15, 245, 153, 136, 228, 135, 58, 81, 56, 155, 236, 137, 17];
    let viewing_key_2 = vec![142, 187, 124, 240, 227, 194, 242, 163, 65, 252, 62, 9, 196, 54, 58, 192, 154, 230, 242, 64, 194, 142, 245, 128, 4, 71, 143, 230, 101, 245, 91, 187];

    let random_1 = vec![218, 149, 98, 132, 226, 15, 222, 160, 140, 137, 58, 102, 160, 218, 201, 109, 131, 176, 227, 205, 123, 164, 238, 6, 60, 83, 17, 43, 94, 209, 252, 184];
    let random_2 = vec![196, 110, 95, 185, 243, 90, 167, 89, 148, 149, 131, 151, 134, 253, 180, 51, 16, 123, 113, 134, 29, 76, 155, 41, 172, 34, 67, 97, 103, 141, 186, 246];
    let random_3 = vec![64, 58, 209, 234, 198, 134, 218, 59, 115, 40, 175, 174, 210, 35, 165, 143, 162, 129, 173, 104, 64, 119, 160, 153, 142, 218, 200, 179, 206, 108, 123, 170];

    let token_id = vec![4, 148, 236, 250, 73, 83, 223, 138, 185, 251, 187, 8, 139, 108, 78, 148, 157, 115, 191, 138, 230, 18, 164, 123, 117, 104, 250, 248, 202, 213, 97, 61];
    let nonce = vec![252, 96, 142, 117, 60, 64, 152, 99, 175, 204, 128, 197];

    let mut tree: MerkleTreeSparse<32> = MerkleTreeSparse::new(0);

    // Add some money to merkle tree
    let utxos_in = vec![
        UTXO::new(
            spending_key_1.clone(),
            viewing_key_1.clone(),
            token_id.clone(),
            random_1.clone(),
            nonce.clone(),
            200,
            "UTXO 1".to_string(),
        ),
        UTXO::new(
            spending_key_1.clone(),
            viewing_key_1.clone(),
            token_id.clone(),
            random_2.clone(),
            nonce.clone(),
            200,
            "UTXO 2".to_string(),
        ),
        UTXO::new(
            spending_key_1.clone(),
            viewing_key_1.clone(),
            token_id.clone(),
            random_3.clone(),
            nonce.clone(),
            200,
            "UTXO 3".to_string(),
        ),
    ];

    let utxos_out = vec![
        UTXO::new(
            spending_key_1.clone(),
            viewing_key_1.clone(),
            token_id.clone(),
            vec![115, 174, 166, 214, 50, 27, 235, 19, 181, 112, 191, 33, 121, 246, 98, 67, 85, 126, 234, 211, 159, 202, 185, 134, 53, 109, 41, 45, 73, 218, 31, 0],
            nonce.clone(),
            300,
            "UTXO 4".to_string(),
        ),
        UTXO::new(
            spending_key_2.clone(),
            viewing_key_2.clone(),
            token_id.clone(),
            vec![115, 174, 166, 214, 50, 27, 235, 19, 181, 112, 191, 33, 121, 246, 98, 67, 85, 126, 234, 211, 159, 202, 185, 134, 53, 109, 41, 45, 73, 218, 31, 1],
            nonce.clone(),
            300,
            "UTXO 5".to_string(),
        ),
    ];

    let commitments: Vec<Vec<u8>> = utxos_in.iter().map(|utxo| utxo.utxo_hash()).collect();
    tree.insert(commitments.clone());

    let mut fake_commitments = vec![];
    for i in 0..8 {
        let hash_i = sha256(vec![&[i]]);
        fake_commitments.push(hash_i);
    }
    tree.insert(fake_commitments);

    // TODO: hash params
    let merkle_root = tree.root();
    let params_hash = keccak(vec![&[100]]);

    let merkle_paths: Vec<Vec<Vec<u8>>> = commitments
        .iter()
        .map(|commitment| tree.generate_proof(commitment.clone()).path)
        .collect();

    let merkle_leaf_indices: Vec<u64> = commitments
        .iter()
        .map(|commitment| tree.generate_proof(commitment.clone()).index as u64)
        .collect();

    let nullifiers: Vec<Vec<u8>> = utxos_in
        .iter()
        .enumerate()
        .map(|(i, utxo_in)| utxo_in.nullifier(merkle_leaf_indices[i]))
        .collect();

    let output_hashes: Vec<Vec<u8>> = utxos_out.iter().map(|utxo| utxo.utxo_hash()).collect();

    let ciphertexts: Vec<CipherText> = utxos_in
        .iter()
        .map(|utxo| utxo.clone().encrypt(viewing_key_1.clone()))
        .collect();
    println!("ciphertexts: {:?}", ciphertexts);

    let pubkey = utxos_in[0].spending_public_key();
    let nullifying_key = utxos_in[0].nullifying_key();
    let signature = utxos_in[0].sign(
        merkle_root.clone(),
        params_hash.clone(),
        nullifiers.clone(),
        output_hashes.clone(),
    );
    let random_inputs = vec![random_1, random_2, random_3];
    let amount_in: Vec<u64> = vec![200, 200, 200];
    let amount_out: Vec<u64> = vec![300, 300];
    let utxo_output_keys: Vec<Vec<u8>> = utxos_out
        .iter()
        .map(|utxo| utxo.utxo_public_key())
        .collect();

    let public_data = PublicData {
        merkle_root: merkle_root.clone(),
        params_hash,
        nullifiers: nullifiers.clone(),
        output_hashes,
    };

    let private_data = PrivateData {
        token_id,
        pubkey,
        signature,
        random_inputs,
        amount_in,
        merkle_paths,
        merkle_leaf_indices,
        nullifying_key,
        utxo_output_keys,
        amount_out,
    };

    let args = Arguments {
        public_data,
        private_data,
        tree_depth: 16u64,
        input_count: 3u64,
        output_count: 2u64,
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
    proof
}