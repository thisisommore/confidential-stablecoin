pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("3eUPqdUMRg26RDBjk4xUNegu8L9kRvQUSccgJsgLqTWB");

#[program]
pub mod token_extensions {
    use super::*;

    pub fn create_mint_with_fee(
        ctx: Context<CreateMintWithFee>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        ctx.accounts
            .create_mint_with_fee(decimals, basis_points, maximum_fee)?;
        Ok(())
    }

    pub fn create_mint_declaritive(
        ctx: Context<CreateMintDeclaritive>,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.create_mint_declarative(decimals)?;
        Ok(())
    }
}
