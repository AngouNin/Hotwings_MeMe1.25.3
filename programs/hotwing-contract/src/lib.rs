use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use spl_associated_token_account::{self, get_associated_token_address};
use pyth_sdk_solana::{load_price, Price};


declare_id!("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow");

/// Constants
const THREE_MONTHS_SECONDS: i64 = 60 * 60 * 24 * 90;
pub const MAX_MILESTONES: usize = 8;
pub const MAX_USERS: usize = 1000;

/// Program module
#[program]
pub mod automated_presale {
    use super::*;

    /// Initialize the program with milestones and setup global state
    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        global_state.token_mint = ctx.accounts.token_mint.key();
        global_state.burn_wallet = ctx.accounts.burn_wallet.key();
        global_state.marketing_wallet = ctx.accounts.marketing_wallet.key();
        global_state.project_wallet = ctx.accounts.project_wallet.key();
        global_state.token_price_oracle = ctx.accounts.token_price_oracle.key();
        global_state.current_market_cap = 0;
        global_state.current_milestone = 0;
        global_state.user_count = 0;
        global_state.three_month_unlock_date = Clock::get()?.unix_timestamp + THREE_MONTHS_SECONDS;

        // Milestones
        global_state.milestones = [
            Milestone { market_cap: 45_000, unlock_percent: 10 },
            Milestone { market_cap: 105_500, unlock_percent: 20 },
            Milestone { market_cap: 225_000, unlock_percent: 30 },
            Milestone { market_cap: 395_000, unlock_percent: 40 },
            Milestone { market_cap: 650_000, unlock_percent: 50 },
            Milestone { market_cap: 997_000, unlock_percent: 60 },
            Milestone { market_cap: 1_574_000, unlock_percent: 70 },
            Milestone { market_cap: 2_500_000, unlock_percent: 100 },
        ];

        Ok(())
    }

    /// Registers multiple users and creates their token accounts (if missing)
    pub fn register_users(ctx: Context<RegisterUsers>, entries: Vec<UserEntry>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Ensure we do not exceed the MAX_USERS constraint
        if global_state.user_count as usize + entries.len() > MAX_USERS {
            return Err(ErrorCode::MaxUsersReached.into());
        }
    
        for entry in entries.iter() {
            // Derive the user's associated token account (ATA)
            let user_ata = get_associated_token_address(&entry.wallet, &global_state.token_mint);
    
            // Create the user's associated token account if it doesn't exist
            if ctx.remaining_accounts.iter().find(|acc| acc.key == &user_ata).is_none() {
                let ata_instruction = spl_associated_token_account::create_associated_token_account(
                    &ctx.accounts.authority.key(),
                    &entry.wallet,
                    &global_state.token_mint,
                );
    
                anchor_lang::solana_program::program::invoke(
                    &ata_instruction,
                    &[
                        ctx.accounts.authority.to_account_info(),
                        ctx.accounts.global_state.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                        ctx.accounts.rent.to_account_info(),
                        ctx.accounts.token_program.to_account_info(),
                    ],
                )?;
                msg!("Created ATA for user: {:?}", entry.wallet);
            }
    
            // Initialize the user's PDA (MilestoneUnlockAccount)
            let user_state = &mut ctx.accounts.user_state;
    
            user_state.wallet = entry.wallet;  // Store user's main wallet address
            user_state.ata = user_ata;         // Store user's ATA address
            user_state.total_locked_tokens = entry.locked_tokens; // Store locked tokens
            user_state.unlocked_tokens = 0;   // Initially, no tokens unlocked
            user_state.last_unlocked_milestone = 0; // No milestones processed
    
            // Increment total user count in global state
            global_state.user_count += 1;
    
            // Emit a message for storage confirmation
            msg!(
                "Registered user: {:?}, Locked Tokens: {}, User ATA: {:?}",
                user_state.wallet,
                user_state.total_locked_tokens,
                user_state.ata
            );
        }
    
        Ok(())
    }

    /// Unlocks tokens automatically when milestone conditions are met
    pub fn process_milestones(ctx: Context<UnlockTokens>, market_cap: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Update current market cap
        global_state.current_market_cap = market_cap;
    
        // Iterate through milestones
        while global_state.current_milestone < global_state.milestones.len() as u8 {
            let milestone = &global_state.milestones[global_state.current_milestone as usize];
    
            // If the current market cap is below the milestone threshold, stop processing
            if market_cap < milestone.market_cap {
                break;
            }
    
            // Log milestone processing
            msg!(
                "Processing milestone {}: Unlock {}%",
                global_state.current_milestone,
                milestone.unlock_percent
            );
    
            // For each user, process token unlocking based on the milestone
            for user_index in 0..global_state.user_count {
                // Derive the user PDA
                let (user_pda, _bump) = Pubkey::find_program_address(
                    &[b"user_state", &user_index.to_le_bytes()],
                    ctx.program_id,
                );
    
                // Fetch user's state account from the remaining accounts provided
                let user_account_info = ctx
                    .remaining_accounts
                    .iter()
                    .find(|account| account.key == &user_pda)
                    .ok_or_else(|| ErrorCode::UserNotFound)?;
    
                let mut user_state: Account<MilestoneUnlockAccount> =
                    Account::try_from(user_account_info)?;
    
                // Unlock tokens based on the milestone's percentage
                let total_unlockable_tokens = user_state
                    .total_locked_tokens
                    .checked_mul(milestone.unlock_percent as u64)
                    .ok_or(ErrorCode::ArithmeticOverflow.into())? // This will now work
                    / 100;
    
                let new_unlock = total_unlockable_tokens - user_state.unlocked_tokens;
    
                if new_unlock > 0 {
                    // Find the user's associated token account (ATA) in the remaining accounts
                    let ata_account_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|account| account.key == &user_state.ata)
                        .ok_or(ErrorCode::UserWalletNotFound)?;
    
                    // Transfer tokens from the project wallet to the user's ATA using a CPI
                    let cpi_accounts = token::Transfer {
                        from: ctx.accounts.project_wallet.to_account_info(),
                        to: ata_account_info.clone(), // Use the fetched AccountInfo for ATA
                        authority: ctx.accounts.project_wallet_authority.to_account_info(),
                    };
    
                    token::transfer(
                        CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
                        new_unlock,
                    )?;
    
                    // Update the user's unlocked token count
                    user_state.unlocked_tokens += new_unlock;
    
                    // Log the unlocking process
                    msg!(
                        "Unlocked {} tokens for user: {:?}",
                        new_unlock,
                        user_state.wallet
                    );
                }
            }
    
            // Move to the next milestone
            global_state.current_milestone += 1;
        }
    
        Ok(())
    }
}

#[account]
pub struct GlobalState {
    pub token_mint: Pubkey,
    pub burn_wallet: Pubkey,
    pub marketing_wallet: Pubkey,
    pub project_wallet: Pubkey,
    pub token_price_oracle: Pubkey,
    pub milestones: [Milestone; MAX_MILESTONES],
    pub current_market_cap: u64,
    pub current_milestone: u8,
    pub user_count: u64,
    pub three_month_unlock_date: i64,
}

// Constants for Milestone size
const MILESTONE_SIZE: usize = 8 + 1; // u64 (8 bytes) + u8 (1 byte)

// Constants for the GlobalState size
impl GlobalState {
    pub const LEN: usize = 8  // Discriminator
        + 32 // token_mint
        + 32 // burn_wallet
        + 32 // marketing_wallet
        + 32 // project_wallet
        + 32 // token_price_oracle
        + (MAX_MILESTONES * MILESTONE_SIZE) // milestones array
        + 8  // current_market_cap
        + 1  // current_milestone
        + 8  // user_count
        + 8; // three_month_unlock_date
}

// User entry structure (used for registering users)
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UserEntry {
    pub wallet: Pubkey, // User's wallet public key
    pub locked_tokens: u64, // Amount of tokens locked during registration
}

#[account]
pub struct MilestoneUnlockAccount {
    pub wallet: Pubkey,                // User's main wallet address
    pub ata: Pubkey,                   // User's associated token account (ATA)
    pub total_locked_tokens: u64,      // Total locked tokens
    pub unlocked_tokens: u64,          // Tokens unlocked so far
    pub last_unlocked_milestone: u8,   // Last milestone processed
}

impl MilestoneUnlockAccount {
    pub const LEN: usize = 8 // Discriminator
        + 32 // wallet
        + 32 // ata
        + 8  // total_locked_tokens
        + 8  // unlocked_tokens
        + 1; // last_unlocked_milestone
}

// Milestone definition
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct Milestone {
    pub market_cap: u64, // Threshold market cap
    pub unlock_percent: u8, // Percentage of tokens unlocked
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(init, payer = authority, space = 8 + GlobalState::LEN)]
    pub global_state: Account<'info, GlobalState>,
    pub token_mint: Account<'info, TokenAccount>,
    pub burn_wallet: Account<'info, TokenAccount>,
    #[account(mut)]
    pub marketing_wallet: Account<'info, TokenAccount>,
    pub project_wallet: Account<'info, TokenAccount>,
    pub token_price_oracle: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterUsers<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state account
    #[account(mut)]
    pub authority: Signer<'info>, // Authority (payer for creating the user state PDA)
    pub rent: Sysvar<'info, Rent>, // Rent system variable
    pub system_program: Program<'info, System>, // System program account
    pub token_program: Program<'info, Token>, // SPL Token program
    /// CHECK: Associated Token Program (unchecked)
    pub associated_token_program: AccountInfo<'info>,
    #[account(
        init, // Initialize the account (user PDA)
        seeds = [b"user_state", authority.key().as_ref()], // Derive PDA using static seed + user key
        bump, // Auto-derive PDA bump
        payer = authority, // Specify who pays for account creation
        space = 8 + MilestoneUnlockAccount::LEN // Allocate space for MilestoneUnlockAccount state
    )]
    pub user_state: Account<'info, MilestoneUnlockAccount>, // User PDA for milestone token tracking
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>,
    pub project_wallet_authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// Error codes
#[error_code]
pub enum ErrorCode {
    #[msg("User already registered")]
    UserAlreadyRegistered,
    #[msg("Max users reached")]
    MaxUsersReached,
    #[msg("Token account creation failed")]
    TokenAccountCreationFailed,
    #[msg("Arithmetic overflow occurred")]
    ArithmeticOverflow,
    #[msg("User not found")]
    UserNotFound, 
    #[msg("User's associated token account (ATA) not found in remaining accounts.")]
    UserWalletNotFound, 
}