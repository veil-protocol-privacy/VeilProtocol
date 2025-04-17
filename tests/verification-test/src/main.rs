
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, compute_budget::ComputeBudgetInstruction, instruction::{AccountMeta, Instruction}, native_token::LAMPORTS_PER_SOL, program_pack::Pack, pubkey, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction::create_account, transaction::Transaction
};

use verification::{process_instruction};
use veil_types::{PublicData, SP1Groth16Proof};

use borsh::{BorshDeserialize, BorshSerialize};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const MULTIPLIER_ELF: &[u8] =  include_bytes!("../bin/multiplier");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rpc_client = RpcClient::new_with_commitment(
        String::from("http://127.0.0.1:8899"),
        CommitmentConfig::confirmed(),
    );
    let verification_program_id = pubkey!("14N3zoByaWcd9YcYUjNKawjLmGBfxDFBpL3iND2aWm2n");
    let mock_invoke_program_id = pubkey!("6MQnjkzjD12Q3JfVD2rxvexZDX9bf7JiLfum17Gag3x9");

    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    // let mut stdin = SP1Stdin::new();
    // let a: u32 = 3;
    // let b: u32 = 4;
    // stdin.write(&a);
    // stdin.write(&b);

    // Setup the program for proving.
    // let (pk, _) = client.setup(MULTIPLIER_ELF);

    // Generate the proof
    // let proof = client
    //     .prove(&pk, &stdin)
    //     .groth16()
    //     .run()
    //     .expect("failed to generate proof");

    // println!("Successfully generated proof!");

    // proof.save("bin/proof.bin").expect("saving proof failed");
    let proof = SP1ProofWithPublicValues::load("verification-test/bin/methods_proof.bin").expect("loading proof failed");

    println!("Proof: {:?}", proof.bytes());
    println!("public values: {:?}", proof.public_values.as_slice());
    let public_data = PublicData::try_from_slice(proof.public_values.as_slice()).expect("failed to parse public data");
    println!("Public data: {:?}", public_data);
    // let proof = SP1Groth16Proof {
    //     proof: proof.bytes().to_vec(),
    //     sp1_public_inputs: vec![12, 0, 0, 0],
    // };

    // let payer = Keypair::new();

    // let transaction_signature = rpc_client
    //     .request_airdrop(&payer.pubkey(), 5 * LAMPORTS_PER_SOL)
    //     .await?;
    // loop {
    //     if rpc_client.confirm_transaction(&transaction_signature).await? {
    //         break;
    //     }
    // }


    // let instruction = Instruction::new_with_borsh(
    //     mock_invoke_program_id,
    //     &proof,
    //     vec![AccountMeta::new(verification_program_id, false)],
    // );

    // let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(500_000u32);

    // let mut transaction = Transaction::new_with_payer(&[cu_ix, instruction], Some(&payer.pubkey()));
    // transaction.sign(&[&payer], rpc_client.get_latest_blockhash().await?);
    // match rpc_client.send_and_confirm_transaction(&transaction).await {
    //     Ok(signature) => println!("Transaction Signature: {}", signature),
    //     Err(err) => eprintln!("Error sending transaction: {}", err),
    // }
    Ok(())
}
// merkle_root: [73, 150, 96, 27, 164, 245, 65, 244, 154, 243, 248, 228, 15, 115, 13, 86, 69, 143, 212, 163, 28, 158, 22, 6, 221, 64, 178, 99, 148, 135, 242, 147]
// nullifiers: [[5, 151, 163, 15, 67, 207, 227, 86, 41, 82, 143, 243, 143, 225, 74, 137, 143, 38, 6, 239, 90, 10, 24, 204, 163, 227, 72, 225, 160, 207, 27, 59], [171, 151, 212, 17, 115, 144, 195, 29, 78, 100, 239, 188, 29, 33, 222, 246, 191, 58, 68, 241, 192, 146, 32, 154, 183, 27, 161, 246, 166, 135, 66, 185], [93, 221, 58, 1, 25, 32, 103, 128, 236, 251, 199, 206, 68, 201, 146, 223, 229, 93, 188, 250, 110, 172, 118, 205, 115, 111, 106, 30, 157, 169, 224, 56]]