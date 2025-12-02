use anchor_lang::prelude::*;

declare_id!("CountR1111111111111111111111111111111111111");

// ============================================================================
// PROGRAM
// ============================================================================

#[program]
pub mod counter {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, initial_count: i64) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = initial_count;
        counter.owner = ctx.accounts.owner.key();
        counter.bump = ctx.bumps.counter;
        Ok(())
    }

    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }

    pub fn decrement(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count -= 1;
        Ok(())
    }

    pub fn reset(ctx: Context<Update>, count: i64) -> Result<()> {
        ctx.accounts.counter.count = count;
        Ok(())
    }
}

// ============================================================================
// ACCOUNTS
// ============================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = owner,
        space = 8 + Counter::INIT_SPACE,
        seeds = [b"counter", owner.key().as_ref()],
        bump
    )]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(
        mut,
        seeds = [b"counter", owner.key().as_ref()],
        bump = counter.bump
    )]
    pub counter: Account<'info, Counter>,
    pub owner: Signer<'info>,
}

// ============================================================================
// STATE
// ============================================================================

#[account]
#[derive(InitSpace)]
pub struct Counter {
    pub count: i64,
    pub owner: Pubkey,
    pub bump: u8,
}
