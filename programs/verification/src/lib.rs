use solana_program::{
    account_info::{next_account_info, AccountInfo}, address_lookup_table::instruction, entrypoint::ProgramResult, msg, program::invoke, program_error::ProgramError, pubkey::Pubkey, system_instruction, sysvar::{rent::Rent, Sysvar}
};
use solana_program::entrypoint;
use borsh::{BorshDeserialize, BorshSerialize};
use groth16_solana::groth16::{Groth16Verifier};
use ark_ff::bytes::{FromBytes, ToBytes};
use std::ops::Neg;

pub mod verifying_key;
use verifying_key::VERIFYINGKEY;

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = Instruction::unpack(instruction_data)?;
    match instruction {
        Instruction::Verify {data} => {
            let proof = Proof::try_from_slice(&data)?;

            let mut proof_a = [0u8; 64];
            proof_a[..64].copy_from_slice(&proof.proof_a[..64]);

            let mut proof_b = [0u8; 128];
            proof_b[..128].copy_from_slice(&proof.proof_b[..128]);
            
            let mut proof_c = [0u8; 64];
            proof_c[..64].copy_from_slice(&proof.proof_c[..64]);

            let mut public_signals = [0u8; 32];
            if proof.inputs.len() < 32 {
                // fill from the last index
                let start = 32 - proof.inputs.len();
                public_signals[start..].copy_from_slice(&proof.inputs[..]);
            } else {
                public_signals[..32].copy_from_slice(&proof.inputs[..32]);
            };

            let result = proof_verifier(proof_a, proof_b, proof_c, public_signals);
            msg!("result: {:?}", result);
        }
    }
    Ok(())
}

// Instructions that our program can execute
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum Instruction {
    Verify { data: Vec<u8> }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Proof {
    pub inputs: Vec<u8>,
    pub proof_a: Vec<u8>,
    pub proof_b: Vec<u8>,
    pub proof_c: Vec<u8>
}


impl Instruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&variant, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        // Match instruction type and parse the remaining bytes based on the variant
        match variant {
            0 => {  
                Ok(Self::Verify {data: rest.to_vec() })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

 /// Verifies a Goth16 zero knowledge proof over the bn254 curve.
 fn proof_verifier(
    mut proof_a: [u8; 64],
    proof_b: [u8; 128],
    proof_c: [u8; 64],
    public_signals: [u8; 32]
) -> bool {
    
    type G1 = ark_ec::short_weierstrass_jacobian::GroupAffine<ark_bn254::g1::Parameters>;
    fn change_endianness(bytes: &[u8]) -> Vec<u8> {
        let mut vec = Vec::new();
        for b in bytes.chunks(32) {
            for byte in b.iter().rev() {
                vec.push(*byte);
            }
        }
        vec
    }

    let proof_a_neg_g1: G1 = <G1 as FromBytes>::read(
        &*[&change_endianness(proof_a.as_slice())[..], &[0u8][..]].concat(),
    )
    .unwrap();
    let mut proof_a_neg = [0u8; 65];
    <G1 as ToBytes>::write(&proof_a_neg_g1.neg(), &mut proof_a_neg[..]).unwrap();
    let proof_a_neg = change_endianness(&proof_a_neg[..64]).try_into().unwrap();
    
    let public_inputs: [[u8; 32]; 1] = [public_signals];
    proof_a = proof_a_neg;
    let mut verifier = Groth16Verifier::new(
        &proof_a,
        &proof_b,
        &proof_c,
        &public_inputs,
        &VERIFYINGKEY,
    )
    .unwrap();
    verifier.verify().unwrap()
}

