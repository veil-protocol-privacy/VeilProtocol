//! Error types

use solana_program::{decode_error::DecodeError, program_error::ProgramError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DarksolError {
    // 0
    /// Failed to create commitments hash.
    FailedCreateCommitmentHash,

    // 1
    /// Failed to insert commitments hash to merkel tree.
    FailedInsertCommitmentHash,

    // 2
    /// Invalid instructions data
    InvalidInstructionData,

    // 3
    /// Invalid request
    InvalidRequest,

    // 4
    /// commitments manager account already initialized
    AccountAlreadyInitialized,

    // 5
    /// UTXO already spent
    UtxoAlreadySpent,

    // 6
    /// invalid merkel root
    InvalidMerkelRoot,

    // 7
    /// nullifiers not match
    NullifiersNotMatch,

    // 8
    /// merkle root not match
    MerkleRootNotMatch,

    // 9
    /// precommitments hash not match
    PreCommitmentHashNotMatch,
}

impl From<DarksolError> for ProgramError {
    fn from(e: DarksolError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for DarksolError {
    fn type_of() -> &'static str {
        "DarksolError"
    }
}
