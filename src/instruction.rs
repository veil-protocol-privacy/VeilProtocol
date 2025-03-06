// Instructions that our program can execute
#[derive(Debug)]
pub enum DarkSolInstruction {
    Deposit,
    Transfer,
    Withdraw,
}

impl DarkSolInstruction {
    // pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
    //     Ok((self));
    // }
}