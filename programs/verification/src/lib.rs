use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};
use utils::VerificationError;
use veil_types::SP1Groth16Proof;

pub mod verify_proof;
use verify_proof::verify_proof;
pub mod utils;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

#[cfg(not(doctest))]
/// Derived as follows:
///
/// ```
/// let client = sp1_sdk::ProverClient::new();
/// let (pk, vk) = client.setup(YOUR_ELF_HERE);
/// let vkey_hash = vk.bytes32();
/// ```
const METHOD_VKEY_HASH: &str =
    "0x00ce4403d3b7f80a5787cf33d4809b5c0ddabf363c8375afd3822fdedc593cbe";

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Deserialize the SP1Groth16Proof from the instruction data.
    let groth16_proof = SP1Groth16Proof::try_from_slice(instruction_data)
        .map_err(|_| VerificationError::InvalidInstructionData)?;

    // Get the SP1 Groth16 verification key from the `sp1-solana` crate.
    let vk = crate::verify_proof::GROTH16_VK_4_0_0_RC3_BYTES;

    // Verify the proof.
    verify_proof(
        &groth16_proof.proof,
        &groth16_proof.sp1_public_inputs,
        &METHOD_VKEY_HASH,
        vk,
    )?;
    Ok(())
}