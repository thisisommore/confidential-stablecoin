use anchor_lang::prelude::*;

#[error_code]
pub enum MintError {
    #[msg("extension is not supported")]
    UnsupportedExtension,
    #[msg("Counter has reached the maximum value")]
    CounterOverflow,
}
