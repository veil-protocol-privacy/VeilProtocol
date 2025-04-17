
use sp1_sdk::{ProverClient, SP1Stdin};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program_pack::Pack,
    pubkey::{self, Pubkey},
    signature::Keypair,
    signer::Signer,
    system_instruction::create_account,
    transaction::Transaction,
};

use verification::{SP1Groth16Proof, process_instruction};

use borsh::{BorshDeserialize, BorshSerialize};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const MULTIPLIER_ELF: &[u8] =  include_bytes!("../elf/multiplier");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rpc_client = RpcClient::new_with_commitment(
        String::from("http://127.0.0.1:8899"),
        CommitmentConfig::confirmed(),
    );
    let program_id = Pubkey::new_unique();

    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&3);
    stdin.write(&4);

    // Setup the program for proving.
    let (pk, _) = client.setup(MULTIPLIER_ELF);

    // Generate the proof
    let proof = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("failed to generate proof");

    println!("Successfully generated proof!");

    proof.save("../proof.bin").expect("saving proof failed");
    println!("Proof: {:?}", proof.bytes());
    println!("public values: {:?}", proof.public_values);
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
        program_id,
        &proof,
        vec![AccountMeta::new(payer.pubkey(), true)],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], rpc_client.get_latest_blockhash().await?);
    match rpc_client.send_and_confirm_transaction(&transaction).await {
        Ok(signature) => println!("Transaction Signature: {}", signature),
        Err(err) => eprintln!("Error sending transaction: {}", err),
    }
    Ok(())
}
