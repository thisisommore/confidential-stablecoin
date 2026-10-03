pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("GJTVX6e1Xu6JtE4BPvfN7b6ED8ZLcZ4u7eb58sRMQ1VJ");

#[program]
pub mod token_extensions {
    use super::*;

    pub fn create_mint(
        ctx: Context<CreateMint>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        create_mint_ix(
            ctx,
            decimals,
            basis_points,
            maximum_fee,
            [
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 1,
            ],
        )?;
        Ok(())
    }
}
