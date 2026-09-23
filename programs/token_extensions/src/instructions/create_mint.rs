use anchor_lang::prelude::*;
use anchor_spl::token_2022::{initialize_mint2, InitializeMint2};
use anchor_spl::token_interface::{
    mint_close_authority_initialize, transfer_fee_initialize, Mint, MintCloseAuthorityInitialize,
    TransferFeeInitialize,
};

use crate::error::MintError;
use crate::{constants::*, state::Counter};
use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions, ExtensionType, StateWithExtensions,
};
use anchor_spl::{self, token_interface::TokenInterface};

const SUPPORTED_EXTENSIONS: &[ExtensionType] = &[
    ExtensionType::MintCloseAuthority,
    ExtensionType::MetadataPointer,
    ExtensionType::TransferFeeConfig,
];
use anchor_spl::token_2022::spl_token_2022::state::Mint as MintState;
#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateMintDeclaritive<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(init,
        payer=payer,
        mint::decimals = decimals, mint::authority = payer, mint::token_program = token_program,
        extensions::close_authority::authority = payer,
        extensions::metadata_pointer::authority = payer,
        extensions::metadata_pointer::metadata_address = payer,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateMintWithFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: no need haha
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AssertSupportedMints<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: no need haha
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> CreateMintDeclaritive<'info> {
    pub fn create_mint_declarative(&self, decimals: u8) -> Result<()> {
        msg!(
            "mint {} created with {} decimals",
            self.mint.key(),
            decimals
        );
        Ok(())
    }
}

impl<'info> CreateMintWithFee<'info> {
    pub fn create_mint_with_fee(
        &self,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        let extensions = [
            ExtensionType::MintCloseAuthority,
            ExtensionType::TransferFeeConfig,
        ];

        let space = ExtensionType::try_calculate_account_len::<MintState>(&extensions)?;
        let lamports = Rent::get()?.minimum_balance(space);

        anchor_lang::system_program::create_account(
            CpiContext::new(
                self.system_program.key(),
                anchor_lang::system_program::CreateAccount {
                    from: self.payer.to_account_info(),
                    to: self.mint.to_account_info(),
                },
            ),
            lamports,
            space as u64,
            &self.token_program.key(),
        )?;

        mint_close_authority_initialize(
            CpiContext::new(
                self.token_program.key(),
                MintCloseAuthorityInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(&self.payer.key()),
        )?;

        transfer_fee_initialize(
            CpiContext::new(
                self.token_program.key(),
                TransferFeeInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(&self.payer.key()),
            Some(&self.payer.key()),
            basis_points,
            maximum_fee,
        )?;

        initialize_mint2(
            CpiContext::new(
                self.token_program.key(),
                InitializeMint2 {
                    mint: self.mint.to_account_info(),
                },
            ),
            decimals,
            &self.payer.key(),
            None,
        )
    }
}

impl<'info> AssertSupportedMints<'info> {
    pub fn assert_supported_mint(&self) -> Result<()> {
        let account_info = self.mint.to_account_info();
        let data = account_info.try_borrow_data()?;
        let state = StateWithExtensions::<MintState>::unpack(&data)?;
        let decimals = state.base.decimals;

        for extension in state.get_extension_types()? {
            require!(
                SUPPORTED_EXTENSIONS.contains(&extension),
                MintError::UnsupportedExtension
            );
        }

        let basis_points = match state.get_extension::<TransferFeeConfig>() {
            Ok(config) => u16::from(
                config
                    .get_epoch_fee(Clock::get()?.epoch)
                    .transfer_fee_basis_points,
            ),
            Err(_) => 0,
        };

        Ok(())
    }
}
