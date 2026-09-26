use anchor_lang::prelude::program::invoke;
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::extension::confidential_transfer_fee::instruction::initialize_confidential_transfer_fee_config;
use anchor_spl::token_2022::spl_token_2022::instruction::initialize_permanent_delegate;
use anchor_spl::token_2022::{initialize_mint2, thaw_account, InitializeMint2, ThawAccount};
use anchor_spl::token_interface::{
    confidential_transfer_fee, default_account_state_initialize, metadata_pointer_initialize,
    mint_close_authority_initialize, transfer_checked_with_fee, transfer_fee_initialize,
    DefaultAccountStateInitialize, MetadataPointerInitialize, Mint, MintCloseAuthorityInitialize,
    TransferCheckedWithFee, TransferFeeInitialize,
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
use anchor_spl::token_2022::spl_token_2022::state::{AccountState, Mint as MintState};

#[derive(Accounts)]
pub struct CreateMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: no need haha
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnFreeze<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: no need haha
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> CreateMint<'info> {
    pub fn create_mint_with_fee(
        &self,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
        wp: [u8; 32],
    ) -> Result<()> {
        let extensions = [
            ExtensionType::TransferFeeConfig,
            ExtensionType::MetadataPointer,
            ExtensionType::DefaultAccountState,
            ExtensionType::MintCloseAuthority,
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferFeeConfig,
            ExtensionType::PermanentDelegate,
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

        metadata_pointer_initialize(
            CpiContext::new(
                self.token_program.key(),
                MetadataPointerInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(self.payer.key()),
            Some(self.mint.key()),
        )?;

        default_account_state_initialize(
            CpiContext::new(
                self.token_program.key(),
                DefaultAccountStateInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            &AccountState::Frozen,
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
        let mint_info = self.mint.to_account_info();
        let program_info = self.token_program.to_account_info();
        let infos = [mint_info.clone(), program_info.clone()];

        invoke(
            &initialize_confidential_transfer_fee_config(
                &self.token_program.key(),
                &self.mint.key(),
                Some(self.mint.key()),
                &wp.into(),
            )?,
            &infos,
        )?;
        invoke(
            &initialize_permanent_delegate(
                &self.token_program.key(),
                &self.mint.key(),
                &self.mint.key(),
            )?,
            &infos,
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

impl<'info> UnFreeze<'info> {
    pub fn unfreeze(&self) -> Result<()> {
        let cpi_accounts = ThawAccount {
            account: self.mint.to_account_info(),
            authority: self.mint.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        thaw_account(CpiContext::new(self.token_program.key(), cpi_accounts))?;
        Ok(())
    }
}
