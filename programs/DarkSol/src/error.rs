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
