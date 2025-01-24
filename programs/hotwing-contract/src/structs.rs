use anchor_lang::prelude::*;
use anchor_spl::token::{ Mint, Token, TokenAccount };
use anchor_spl::token_interface::Token2022;
use crate::consts::*;
use crate::errors::ErrorCode;

#[account]
#[derive(Default)] // Add default implementation for easier initialization
pub struct GlobalState {
    ///CHECK: Admin authority
    pub authority: Pubkey,                        // Admin authority (32 bytes)
    ///CHECK: Token mint account
    pub token_mint: Pubkey,                       // Token mint address (32 bytes)
    /// CHECK: This is a standard wallet account, and the program will verify its usage.
    pub burn_wallet: Pubkey,           
    /// CHECK: This is a standard wallet account, and the program will verify its usage.
    pub marketing_wallet: Pubkey,     
    /// CHECK: This is a standard wallet account, and the program will verify its usage.                // Marketing wallet account (32 bytes)
    pub project_wallet: Pubkey,                   // Project wallet account (32 bytes)
    pub milestones: [Milestone; MAX_MILESTONES],  // Milestone data (variable size)
    pub current_market_cap: u64,                  // Current market cap (8 bytes)
    pub current_milestone: u8,                    // Current milestone index (1 byte)
    pub user_count: u64,                          // Total user count (8 bytes)
    pub three_month_unlock_date: i64,             // Unlock date (3 months) (8 bytes)
    pub exempted_wallets: Vec<Pubkey>,            // List of exempt wallets (dynamic size - limit required!)
    pub unlock_complete: bool,                    // Full unlock flag (1 byte) 
    pub raydium_program_id: Pubkey,               // Program ID of Raydium AMM (32 bytes)
    pub liquidity_pool: Pubkey,                   // Raydium liquidity pool account (32 bytes)
}



// Constants for the GlobalState size
impl GlobalState {
    // Calculate the size of the GlobalState struct
    pub const LEN: usize = 8
        + 32                           // authority
        + 32                                            // token_mint
        + 32                                            // burn_wallet
        + 32                                            // marketing_wallet
        + 32                                            // project_wallet
        + (MILESTONE_SIZE * MAX_MILESTONES)       // Fixed-size milestones
        + 8                                             // current_market_cap
        + 1                                             // current_milestone
        + 8                                             // user_count
        + 8                                             // three_month_unlock_date
        + 4 + (32 * MAX_EXEMPTED_WALLETS)         // Exempted_wallets (Vec metadata + max size)
        + 1                                            // unlock_complete flag
        + 32
        + 32; 
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(init, payer = authority, space = 8 + GlobalState::LEN)]
    pub global_state: Account<'info, GlobalState>,
    /// The Token Mint account
    pub token_mint: Account<'info, Mint>, // Correct type for SPL Token Mint
    /// The Burn Wallet
    /// CHECK: This must be a standard wallet. Use constraints to validate.
    #[account(
        constraint = burn_wallet.lamports() > 0 @ ErrorCode::InvalidBurnWallet,  // Ensure account exists
        constraint = *burn_wallet.owner == solana_program::system_program::ID @ ErrorCode::InvalidBurnWallet // Ensure it's not owned by another program
    )]
    pub burn_wallet: AccountInfo<'info>, // Burn wallet

    /// The Marketing Wallet
    /// CHECK: This must be a standard wallet. Use constraints to validate.
    #[account(
        constraint = marketing_wallet.lamports() > 0 @ ErrorCode::InvalidMarketingWallet, // Ensure account exists
        constraint = *marketing_wallet.owner == solana_program::system_program::ID @ ErrorCode::InvalidMarketingWallet // Ensure it's not owned by another program
    )]
    pub marketing_wallet: AccountInfo<'info>, // Marketing wallet
    /// The Project Wallet
    /// CHECK: This must be a standard wallet. Use constraints to validate.
    #[account(
        constraint = project_wallet.lamports() > 0 @ ErrorCode::InvalidProjectWallet, // Ensure account exists
        constraint = *project_wallet.owner == solana_program::system_program::ID @ ErrorCode::InvalidProjectWallet // Ensure it's not owned by another program
    )]
    pub project_wallet: AccountInfo<'info>, // Project wallet
    /// Admin authority
    #[account(mut)]
    pub authority: Signer<'info>, // Admin authority
    /// System program
    pub system_program: Program<'info, System>, // System Program
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
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct Milestone {
    pub market_cap: u64, // Threshold market cap
    pub unlock_percent: u8, // Percentage of tokens unlocked
}

#[derive(Accounts)]
pub struct RegisterUsers<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state account
    #[account(mut)]
    ///CHECK: This is a standard wallet account, and the program will verify its usage
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
     ///CHECK: User state PDA account
    pub user_state: Account<'info, MilestoneUnlockAccount>, // User PDA for milestone token tracking
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>,
    pub project_wallet_authority: Signer<'info>,
    #[account(mut)]
    pub burn_wallet: Account<'info, TokenAccount>,
    #[account(mut)]
    pub marketing_wallet: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)] 
pub struct ManageExemptWallet<'info> {
    #[account(
        mut,
        has_one = authority, // Verify that the provided `Signer` matches the stored `authority`
        constraint = authority.key() == global_state.authority @ ErrorCode::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>, // Global configuration
    ///CHECK: This is a standard wallet account, and the program will verify its usage
    pub authority: Signer<'info>, // Admin authority to sign the transaction
}

#[derive(Accounts)]
pub struct UpdateMarketCap<'info> {
    #[account(
        mut,
        has_one = authority, // Verify that the provided `Signer` matches the stored `authority`
        constraint = authority.key() == global_state.authority @ ErrorCode::Unauthorized
    )]
    pub global_state: Account<'info, GlobalState>, // Global state account
    ///CHECK: This is a standard wallet account, and the program will verify its usage
    pub authority: Signer<'info>,                 // Signer (admin authority)
}

#[derive(Accounts)]
pub struct RegisterUserOnTransfer<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state account that stores global data
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>, // Project wallet receiving locked tokens
    #[account(mut)]
    ///CHECK: This is a standard wallet account, and the program will verify its usage
    pub authority: Signer<'info>, // The signer who calls this transaction (payer for accounts)
    pub rent: Sysvar<'info, Rent>, // Rent sysvar
    pub system_program: Program<'info, System>,  // System program (for creating PDAs)
    pub token_program: Program<'info, Token>, // SPL Token Program for token transfers
    /// CHECK: Associated Token Program
    pub associated_token_program: AccountInfo<'info>,
    #[account(mut)] // User state PDA manually validated inside the function
    ///CHECK: User state PDA account
    pub user_state: AccountInfo<'info>, // User-specific PDA account
}

#[derive(Accounts)]
pub struct RegisterHook<'info> {
    /// The Token-2022 mint account (target for the transfer hook update).
    #[account(mut)]
    ///CHECK: Token mint account
    pub token_mint: AccountInfo<'info>,
    /// The authority on the Token-2022 mint (must sign the CPI).
    #[account(signer)]
    ///CHECK: The authority who triggered the update
    pub authority: AccountInfo<'info>,
    /// The SPL Token-2022 program.
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct UpdateRaydiumProgramId<'info> {
    #[account(
        mut,
        has_one = authority @ ErrorCode::Unauthorized // Ensure authority matches the one in GlobalState
    )]
    pub global_state: Account<'info, GlobalState>, // Global state account
    ///CHECK: This is a standard wallet account, and the program will verify its usage
    pub authority: Signer<'info>, // Admin authority
}

#[derive(Accounts)]
pub struct UpdateLiquidityPoolAddress<'info> {
    #[account(
        mut,
        has_one = authority @ ErrorCode::Unauthorized, // Check that the provided authority matches
    )]
    pub global_state: Account<'info, GlobalState>, // Global State
    #[account(signer)]
    ///CHECK: The signer is the program's caller
    pub authority: Signer<'info>, // Admin signer to approve such changes
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    #[account(
        mut,
        has_one = authority @ ErrorCode::Unauthorized // Verify relationship
    )]
    pub global_state: Account<'info, GlobalState>, // Global State
    #[account(signer)]
    ///CHECK: Signer ensures the caller is verified
    pub authority: AccountInfo<'info>, // Current authority
}

pub struct RaydiumTransactionHelper;

impl RaydiumTransactionHelper {
    /// Checks if the source account belongs to Raydium's Program
    pub fn is_raydium_transaction(source_account: &AccountInfo, raydium_program_id: Pubkey) -> bool {
        // Check if the source account's owner matches Raydium's program ID
        if source_account.owner == &raydium_program_id {
            msg!("Raydium transaction detected for source account: {:?}", source_account.key);
            return true;
        }

        // Log for non-Raydium transactions (optional, for debugging)
        msg!(
            "Source account {:?} is not owned by Raydium. Owner: {:?}",
            source_account.key,
            source_account.owner
        );

        return false;
    }
}