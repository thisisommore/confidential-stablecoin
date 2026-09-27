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

pub fn create_mint_ix(
    ctx: Context<CreateMint>,
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
            ctx.accounts.system_program.key(),
            anchor_lang::system_program::CreateAccount {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        lamports,
        space as u64,
        &ctx.accounts.token_program.key(),
    )?;

    transfer_fee_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferFeeInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
        Some(&ctx.accounts.payer.key()),
        basis_points,
        maximum_fee,
    )?;

    metadata_pointer_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MetadataPointerInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(ctx.accounts.payer.key()),
        Some(ctx.accounts.mint.key()),
    )?;

    default_account_state_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            DefaultAccountStateInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        &AccountState::Frozen,
    )?;

    mint_close_authority_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MintCloseAuthorityInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
    )?;
    let mint_info = ctx.accounts.mint.to_account_info();
    let infos = [mint_info.clone()];

    invoke(
        &initialize_confidential_transfer_fee_config(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()),
            &wp.into(),
        )?,
        &infos,
    )?;
    invoke(
        &initialize_permanent_delegate(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.payer.key(),
        )?,
        &infos,
    )?;

    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            InitializeMint2 {
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        decimals,
        &ctx.accounts.payer.key(),
        Some(&ctx.accounts.payer.key()),
    )
}

pub fn unfreeze_ix(ctx: Context<UnFreeze>) -> Result<()> {
    let cpi_accounts = ThawAccount {
        account: ctx.accounts.mint.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };
    thaw_account(CpiContext::new(
        ctx.accounts.token_program.key(),
        cpi_accounts,
    ))?;
    Ok(())
}
