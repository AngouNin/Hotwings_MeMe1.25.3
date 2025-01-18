use anchor_lang::prelude::*;
use anchor_spl::token::{self, spl_token, Token, TokenAccount, Transfer};
use pyth_sdk_solana::{load_price, Price};

declare_id!("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow"); // Replace this with the deployed program ID after building

/// Constants
const THREE_MONTHS_SECONDS: i64 = 60 * 60 * 24 * 90; // 3 months
const MAX_USER: usize = 1000; // Maximum number of users
const MAX_WALLET_CAP: u64 = 50_000_000; // Max wallet tokens for anti-whale logic (5% of total supply)
pub const MAX_MILESTONES: usize = 8; // Example max number of milestones

/// Program entry point
#[program]
pub mod hotwings_token {
    use super::*;

    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Set initial configuration
        global_state.token_mint = ctx.accounts.token_mint.key();
        global_state.burn_wallet = ctx.accounts.burn_wallet.key();
        global_state.marketing_wallet = ctx.accounts.marketing_wallet.key();
        global_state.project_wallet = ctx.accounts.project_wallet.key();
        global_state.token_price_oracle = ctx.accounts.token_price_oracle.key();
        global_state.current_market_cap = 0;
        global_state.current_milestone = 0;
        global_state.three_month_unlock_date = Clock::get()?.unix_timestamp + THREE_MONTHS_SECONDS;
    
        // Define milestones directly without using dynamic logic
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

    /// Locks tokens during the presale phase for users
    pub fn lock_presale_tokens(ctx: Context<LockTokens>, locked_amount: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Validate maximum users
        if global_state.user_count >= MAX_USER as u64 {
            return Err(ProgramError::Custom(1).into()); // "User limit reached"
        }
    
        // Check if the user's PDA account already exists (duplicate check)
        if ctx.accounts.user_state.to_account_info().lamports() > 0 {
            return Err(ProgramError::Custom(5).into()); // "User already registered"
        }
    
        // Create PDA for the user
        let user_state = &mut ctx.accounts.user_state;
        user_state.wallet = ctx.accounts.user_wallet.key(); // Link the user's wallet
        user_state.total_locked_tokens = locked_amount;
        user_state.unlocked_tokens = 0;
        user_state.last_unlocked_milestone = 0;
    
        // Increment user count in global state
        global_state.user_count += 1;
    
        Ok(())
    }

    /// Unlocks tokens for users when market cap milestones are reached
    pub fn unlock_tokens(ctx: Context<UnlockTokens>, market_cap: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Update the market cap
        global_state.current_market_cap = market_cap;
    
        // Unlock tokens based on milestones
        while global_state.current_milestone < global_state.milestones.len() as u8 {
            let milestone = &global_state.milestones[global_state.current_milestone as usize];
    
            // Break if we haven't reached the current milestone
            if market_cap < milestone.market_cap {
                break;
            }
    
            // Iterate over all user states (by fetching PDAs programmatically)
            // This assumes PDAs are generated using `[b"user_state", wallet_address]`
            for i in 0..global_state.user_count {
                let seed = [b"user_state", &i.to_le_bytes()];
                if let Some(user_state) = get_user_state(&ctx.accounts, seed)? {
                    // Calculate total unlockable tokens
                    let total_unlockable = user_state
                        .total_locked_tokens
                        .checked_mul(milestone.unlock_percent as u64)
                        .ok_or(ProgramError::Custom(2))? / 100;
    
                    // Calculate new tokens to unlock
                    let new_unlock = total_unlockable - user_state.unlocked_tokens;
    
                    // Anti-whale protection
                    if total_unlockable > MAX_WALLET_CAP {
                        return Err(ProgramError::Custom(3).into()); // Anti-whale triggered
                    }
    
                    // Perform the token transfer
                    // (Token accounts should be deserialized for SPL transfer here)
                    if new_unlock > 0 {
                        token::transfer(
                            CpiContext::new(
                                ctx.accounts.token_program.to_account_info(),
                                token::Transfer {
                                    from: ctx.accounts.project_wallet.to_account_info(),
                                    to: user_state.wallet.to_account_info(), // user's token account
                                    authority: ctx.accounts.project_wallet_authority.to_account_info(),
                                },
                            ),
                            new_unlock,
                        )?;
    
                        // Update user's unlocked tokens
                        user_state.unlocked_tokens += new_unlock;
                    }
                }
            }
    
            // Increment to the next milestone
            global_state.current_milestone += 1;
        }
    
        Ok(())
    }
    
    

    /// Fetches token price from oracle to update market cap
    pub fn update_market_cap(ctx: Context<UpdateMarketCap>, total_supply: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // Fetch the current price from the oracle
        let price_data: Price = load_price(&ctx.accounts.token_price_oracle)?;

        // Get the current price in USD
        let current_price: u64 = price_data.price as u64;

        // Calculate market cap
        global_state.current_market_cap = current_price
            .checked_mul(total_supply)
            .ok_or(ProgramError::Custom(4))?;

        Ok(())
    }
}


// Helper function to fetch a user state from PDA
fn get_user_state(ctx: &Context<UnlockTokens>, seed: &[u8]) -> Option<Account<MilestoneUnlockAccount>> {
    let user_pda = Pubkey::find_program_address(seed, ctx.program_id);
    Account::try_from(&ctx.remaining_accounts.iter().find(|acc| acc.key == user_pda).unwrap()).ok()
}

#[account]
pub struct GlobalState {
    pub token_mint: Pubkey,                     // Token Mint
    pub burn_wallet: Pubkey,                   // Burn Wallet to burn excess tokens
    pub marketing_wallet: Pubkey,              // Marketing Wallet for promotional tokens
    pub project_wallet: Pubkey,                // Project Wallet holding tokens to unlock
    pub token_price_oracle: Pubkey,            // Pyth Oracle to fetch token price
    pub milestones: [Milestone; MAX_MILESTONES], // Array of milestones
    pub current_market_cap: u64,               // Current market cap of the token
    pub current_milestone: u8,                 // Index of the current milestone
    pub three_month_unlock_date: i64,          // Unlock all after 3 months (fallback)
    pub user_count: u64,                       // Number of users registered in presale
}
impl GlobalState {
    pub const LEN: usize =
        8 +  // Discriminator
        32 + // token_mint
        32 + // burn_wallet
        32 + // marketing_wallet
        32 + // project_wallet
        32 + // token_price_oracle
        (MAX_MILESTONES * Milestone::LEN) + // milestones fixed size
        8 +  // current_market_cap
        1 +  // current_milestone
        8 +  // three_month_unlock_date
        8;   // user_count
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct Milestone {
    pub market_cap: u64,  // Market cap threshold for this milestone
    pub unlock_percent: u8, // Percentage of tokens unlocked at this milestone
}
impl Milestone {
    pub const LEN: usize = 8 + 1; // u64 + u8
}

#[account]
pub struct MilestoneUnlockAccount {
    pub wallet: Pubkey,                // User's wallet public key
    pub total_locked_tokens: u64,      // Total amount of tokens locked during presale for this user
    pub unlocked_tokens: u64,          // Total number of tokens unlocked by this user so far
    pub last_unlocked_milestone: u8,   // Index of the last milestone unlocked for this user
}
impl MilestoneUnlockAccount {
    pub const LEN: usize =
        8 +  // Discriminator
        32 + // wallet
        8 +  // total_locked_tokens
        8 +  // unlocked_tokens
        1;   // last_unlocked_milestone
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(init, payer = authority, space = 8 + GlobalState::LEN)] // Allocate space for GlobalState
    pub global_state: Account<'info, GlobalState>,
    #[account()]
    pub token_mint: Account<'info, TokenAccount>,       // Token mint account
    #[account()]
    pub burn_wallet: Account<'info, TokenAccount>,      // Burn wallet
    #[account()]
    pub marketing_wallet: Account<'info, TokenAccount>, // Marketing wallet
    #[account()]
    pub project_wallet: Account<'info, TokenAccount>,   // Program's token wallet
    pub token_price_oracle: AccountInfo<'info>,         // Pyth price oracle account
    #[account(mut)]
    pub authority: Signer<'info>,                      // Signer initializing the program
    pub system_program: Program<'info, System>,         // System Program
}

#[derive(Accounts)]
pub struct LockTokens<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state account managing the program
    #[account(
        init, 
        payer = user_wallet, 
        space = 8 + MilestoneUnlockAccount::LEN
    )]
    pub user_state: Account<'info, MilestoneUnlockAccount>, // PDA for the user's locking data
    #[account(mut)]
    pub user_wallet: Signer<'info>, // Presale participant's wallet
    pub system_program: Program<'info, System>, // Required for account initializations
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state containing milestone data
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>, // Program's wallet holding locked tokens
    pub project_wallet_authority: Signer<'info>,      // Authority to sign transfers
    pub token_program: Program<'info, Token>,         // SPL-Token program
}

#[derive(Accounts)]
pub struct UpdateMarketCap<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account()]
    pub token_price_oracle: AccountInfo<'info>,
}