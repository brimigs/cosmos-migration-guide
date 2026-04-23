use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
    },
};

declare_id!("E5cQQYxz654NddXsQonWyZLBqp9AgWWSppiLfSZ4Rm1j");

#[program]
pub mod escrow {
    use super::*;

    pub fn create_escrow(ctx: Context<CreateEscrow>, escrow_seed: Pubkey) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        escrow.version = 1;
        escrow.bump = ctx.bumps.escrow;
        escrow.escrow_seed = escrow_seed;
        escrow.admin = ctx.accounts.admin.key();
        Ok(())
    }

    pub fn allow_mint(ctx: Context<AllowMint>) -> Result<()> {
        let allowed_mint = &mut ctx.accounts.allowed_mint;
        allowed_mint.version = 1;
        allowed_mint.bump = ctx.bumps.allowed_mint;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, receipt_seed: Pubkey, amount: u64) -> Result<()> {
        require!(amount > 0, EscrowError::ZeroDepositAmount);

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.depositor_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;

        let receipt = &mut ctx.accounts.receipt;
        receipt.version = 1;
        receipt.bump = ctx.bumps.receipt;
        receipt.escrow = ctx.accounts.escrow.key();
        receipt.depositor = ctx.accounts.depositor.key();
        receipt.mint = ctx.accounts.mint.key();
        receipt.receipt_seed = receipt_seed;
        receipt.amount = amount;
        receipt.deposited_at = Clock::get()?.unix_timestamp;

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        let bump = [escrow.bump];
        let signer_seeds: &[&[&[u8]]] = &[&[b"escrow", escrow.escrow_seed.as_ref(), &bump]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.withdrawer_token_account.to_account_info(),
                    authority: ctx.accounts.escrow.to_account_info(),
                },
                signer_seeds,
            ),
            ctx.accounts.receipt.amount,
            ctx.accounts.mint.decimals,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(escrow_seed: Pubkey)]
pub struct CreateEscrow<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [b"escrow", escrow_seed.as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AllowMint<'info> {
    #[account(
        has_one = admin,
        seeds = [b"escrow", escrow.escrow_seed.as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,
    pub admin: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        space = 8 + AllowedMint::INIT_SPACE,
        seeds = [b"allowed_mint", escrow.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub allowed_mint: Account<'info, AllowedMint>,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(receipt_seed: Pubkey)]
pub struct Deposit<'info> {
    #[account(
        seeds = [b"escrow", escrow.escrow_seed.as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        seeds = [b"allowed_mint", escrow.key().as_ref(), mint.key().as_ref()],
        bump = allowed_mint.bump
    )]
    pub allowed_mint: Account<'info, AllowedMint>,
    #[account(
        init,
        payer = payer,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [
            b"receipt",
            escrow.key().as_ref(),
            depositor.key().as_ref(),
            mint.key().as_ref(),
            receipt_seed.as_ref()
        ],
        bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        constraint = depositor_token_account.owner == depositor.key() @ EscrowError::InvalidDepositorTokenAccount,
        constraint = depositor_token_account.mint == mint.key() @ EscrowError::InvalidDepositorTokenAccount
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub depositor: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(
        seeds = [b"escrow", escrow.escrow_seed.as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        mut,
        close = rent_recipient,
        has_one = escrow,
        has_one = depositor,
        has_one = mint,
        seeds = [
            b"receipt",
            escrow.key().as_ref(),
            withdrawer.key().as_ref(),
            mint.key().as_ref(),
            receipt.receipt_seed.as_ref()
        ],
        bump = receipt.bump,
        constraint = receipt.depositor == withdrawer.key() @ EscrowError::InvalidWithdrawer
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        constraint = withdrawer_token_account.owner == withdrawer.key() @ EscrowError::InvalidWithdrawerTokenAccount,
        constraint = withdrawer_token_account.mint == mint.key() @ EscrowError::InvalidWithdrawerTokenAccount
    )]
    pub withdrawer_token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Rent recipient for the closed receipt PDA.
    #[account(mut)]
    pub rent_recipient: UncheckedAccount<'info>,
    pub withdrawer: Signer<'info>,
    /// CHECK: Verified by `has_one` on the receipt.
    pub depositor: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub version: u8,
    pub bump: u8,
    pub escrow_seed: Pubkey,
    pub admin: Pubkey,
}

#[account]
#[derive(InitSpace)]
pub struct AllowedMint {
    pub version: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub version: u8,
    pub bump: u8,
    pub escrow: Pubkey,
    pub depositor: Pubkey,
    pub mint: Pubkey,
    pub receipt_seed: Pubkey,
    pub amount: u64,
    pub deposited_at: i64,
}

#[error_code]
pub enum EscrowError {
    #[msg("The depositor token account does not match the expected owner or mint.")]
    InvalidDepositorTokenAccount,
    #[msg("The withdrawer token account does not match the expected owner or mint.")]
    InvalidWithdrawerTokenAccount,
    #[msg("Only the original depositor can withdraw this receipt.")]
    InvalidWithdrawer,
    #[msg("Deposit amount must be greater than zero.")]
    ZeroDepositAmount,
}
