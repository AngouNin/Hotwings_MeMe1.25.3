use anchor_lang::prelude::*;
use anchor_spl::token::{ Mint, Token, TokenAccount}; 
use spl_token::solana_program::program_pack::Pack;


declare_id!("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow");

/// Constants
const THREE_MONTHS_SECONDS: i64 = 60 * 60 * 24 * 90;
pub const MAX_MILESTONES: usize = 8;
pub const MAX_USERS: usize = 1000;
pub const MAX_HOLD_AMOUNT: u64 = 50000000; // Anti-whale restriction:
pub const MAX_EXEMPTED_WALLETS: usize = 128; // Maximum exempted wallets

/// Program module
#[program]
pub mod automated_presale {
    use anchor_spl::{associated_token::{get_associated_token_address, spl_associated_token_account}, token};

    use super::*;

    /// Initialize the program with milestones and setup global state
    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        // Ensure burn_wallet is a valid writable account
        let burn_wallet_account = &ctx.accounts.burn_wallet;
        if !burn_wallet_account.is_writable {
            return Err(ErrorCode::InvalidBurnWallet.into());
            
        }
        let marketing_wallet_account = &ctx.accounts.marketing_wallet;
        if !marketing_wallet_account.is_writable {
            return Err(ErrorCode::InvalidMarketingWallet.into());
        }
        let project_wallet_account = &ctx.accounts.project_wallet;
        if !project_wallet_account.is_writable {
            return Err(ErrorCode::InvalidProjectWallet.into());
        }

        global_state.token_mint = ctx.accounts.token_mint.key(); 
        global_state.burn_wallet = ctx.accounts.burn_wallet.key();
        global_state.marketing_wallet = ctx.accounts.marketing_wallet.key();
        global_state.project_wallet = ctx.accounts.project_wallet.key();
        // global_state.token_price_oracle = ctx.accounts.token_price_oracle.key();
        global_state.current_market_cap = 0;
        global_state.current_milestone = 0;
        global_state.user_count = 0;
        global_state.three_month_unlock_date = Clock::get()?.unix_timestamp + THREE_MONTHS_SECONDS;
        global_state.unlock_complete = false;
        global_state.exempted_wallets = Vec::new();
        msg!("Global state initialized with admin authority: {:?}", global_state.authority);

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

            // First, verify if the user is already registered
            for account_info in ctx.remaining_accounts.iter() {
                if account_info.key == &entry.wallet { // Check if wallet already exists
                    return Err(ErrorCode::UserAlreadyRegistered.into());
                }
            }

            // Derive the user's associated token account (ATA)
            let user_ata = get_associated_token_address(&entry.wallet, &global_state.token_mint);
    
            // Create the user's associated token account if it doesn't exist
            if ctx.remaining_accounts.iter().find(|acc| acc.key == &user_ata).is_none() {
                let ata_instruction = spl_associated_token_account::instruction::create_associated_token_account(
                    &ctx.accounts.authority.key(),
                    &entry.wallet,
                    &global_state.token_mint,
                    &spl_associated_token_account::ID,
                );
    
                anchor_lang::solana_program::program::invoke(
                    &ata_instruction,
                    &[
                        ctx.accounts.authority.to_account_info(),
                        global_state.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                        ctx.accounts.rent.to_account_info(),
                        ctx.accounts.token_program.to_account_info(),
                    ],
                ).map_err(|_| ErrorCode::TokenAccountCreationFailed)?;
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

            // Emit the `UserRegistered` event
            emit!(UserRegistered {
                wallet: entry.wallet,
                locked_tokens: entry.locked_tokens,
                ata: user_ata,
            });
    
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

    pub fn add_exempt_wallet(ctx: Context<ManageExemptWallet>, wallet: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Ensure the wallet is not already in the exempt list
        if !global_state.exempted_wallets.contains(&wallet) {
            global_state.exempted_wallets.push(wallet); // Add wallet to the list
            msg!("Exempted wallet added: {:?}", wallet);
        } else {
            msg!("Wallet is already exempted: {:?}", wallet);
        }
    
        Ok(())
    }

    pub fn remove_exempt_wallet(ctx: Context<ManageExemptWallet>, wallet: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Remove wallet from the exempt list
        let original_count = global_state.exempted_wallets.len();
        global_state.exempted_wallets.retain(|&key| key != wallet);
    
        if original_count > global_state.exempted_wallets.len() {
            msg!("Exempted wallet removed: {:?}", wallet);
        } else {
            msg!("Wallet was not found in the exempt list: {:?}", wallet);
        }
    
        Ok(())
    }

    /// Unlocks tokens automatically when milestone conditions are met
    pub fn process_milestones<'a, 'b, 'c, 'd, 'info>(ctx: Context<'a, 'b, 'c, 'd, UnlockTokens<'info>>, market_cap: u64) -> Result<()>
    where 'c: 'info, 'd: 'info, 'info: 'd {
        let global_state = &mut ctx.accounts.global_state;
    
        // Update current market cap
        global_state.current_market_cap = market_cap;

        // Check if the three-month unlock date has passed
        let current_timestamp = Clock::get()?.unix_timestamp;

        // Ensure full unlock bypasses anti-whale restrictions
        if current_timestamp >= global_state.three_month_unlock_date || global_state.current_market_cap >= 2_500_000 {
            msg!("Unlock conditions met: Anti-whale restrictions lifted. Full unlocking allowed.");
            global_state.unlock_complete = true;

            // Trigger auto-sell logic if not already triggered
            if !global_state.auto_sell_triggered {
                msg!("Triggering auto-sell mechanism...");
                // trigger_auto_sell(&ctx)?; // Call the auto-sell function
                global_state.auto_sell_triggered = true; // Mark auto-sell as completed
            } else {
                msg!("Auto-sell already triggered. Skipping...");
            }
        } else {
            msg!("Unlock conditions not met: Three-month lock period still active.
                All users are subject to anti-whale restrictions.");
            global_state.unlock_complete = false;
        }
    
        let mut current_milestone_index = global_state.current_milestone as usize;
        // Iterate through milestones
        let mut milestone = &global_state.milestones[current_milestone_index];

        if current_milestone_index < MAX_MILESTONES {

            // If the current market cap is below the milestone threshold, stop processing
            if market_cap < milestone.market_cap {
                Err(ErrorCode::MiletoneNotReached)?;
            } else {
                while (current_milestone_index < MAX_MILESTONES)
                    && (market_cap >= global_state.milestones[current_milestone_index].market_cap)
                {
                    current_milestone_index += 1;
                }
                
            }

            // Update the current milestone index
            global_state.current_milestone = current_milestone_index as u8;
            milestone = &global_state.milestones[current_milestone_index];
    
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

                if ctx.remaining_accounts.len() < global_state.user_count as usize * 2 {
                    // Each user must have a PDA and an ATA in remaining accounts
                    return Err(ErrorCode::AccountNotEnough.into());
                }
    
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
                    .ok_or::<anchor_lang::error::Error>(ErrorCode::ArithmeticOverflow.into())? // This will now work
                    / 100;

                // Find the user's associated token account (ATA) in the remaining accounts
                let ata_account_info = ctx
                .remaining_accounts
                .iter()
                .find(|account| account.key == &user_state.ata)
                .ok_or(ErrorCode::UserWalletNotFound)?;

                let mut new_unlock: u64;
                
                if global_state.unlock_complete {
                    new_unlock = user_state.total_locked_tokens;
                    // Full unlock bypasses anti-whale restrictions
                    msg!("Full unlock: {} tokens unlocked for user: {:?}", new_unlock, user_state.wallet);
                } else {
                    // Check if the user is exempted from anti-whale restrictions
                    if global_state.exempted_wallets.contains(&user_state.wallet) {
                        new_unlock = total_unlockable_tokens - user_state.unlocked_tokens;
                        msg!("Exempted wallet: {} tokens unlocked for user: {:?}", new_unlock, user_state.wallet);
                    } else {
                        // Anti-whale restrictions apply
                        new_unlock = total_unlockable_tokens - user_state.unlocked_tokens;
                        // Fetch recipient's current token balance
                        let recipient_balance = get_token_balance(ata_account_info.clone())?;
                        // Calculate post-transfer balance
                        let after_transfer_balance = recipient_balance + new_unlock;
                        
                        if after_transfer_balance > MAX_HOLD_AMOUNT {
                            // Anti-whale restriction: Max 50,000,000 tokens can be held by a user now.
                            let available_unlock = MAX_HOLD_AMOUNT - recipient_balance;
                            new_unlock = available_unlock;
                            msg!("Anti-whale restriction: {} tokens unlocked for user: {:?}", available_unlock, user_state.wallet);
                            
                        }
                    }
                }
                
                if new_unlock > 0 {
                    // Transfer tokens from the project wallet to the user's ATA using a CPI
                    let cpi_accounts = token::Transfer {
                        from: ctx.accounts.project_wallet.to_account_info(),
                        to: ata_account_info.clone(), // Use the fetched AccountInfo for ATA
                        authority: ctx.accounts.project_wallet_authority.to_account_info(),
                    };
    
                    token::transfer(
                        CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
                        new_unlock,
                    ).map_err(|_| ErrorCode::TokenTransferFailed)?;
    
                    // Update the user's unlocked token count
                    user_state.unlocked_tokens += new_unlock;
                    user_state.total_locked_tokens -= new_unlock;
                    user_state.last_unlocked_milestone = global_state.current_milestone;

                    emit!(MilestoneProcessed {
                        wallet: user_state.wallet,
                        unlocked_tokens: new_unlock,
                        milestone_index: global_state.current_milestone,
                    });
    
                    // Log the unlocking process
                    msg!(
                        "Unlocked {} tokens for user: {:?}",
                        new_unlock,
                        user_state.wallet
                    );
                }
            }
    
        }
    
        Ok(())
    }

    pub fn update_market_cap(ctx: Context<UpdateMarketCap>, market_cap: u64) -> Result<()> {
        // Update the current market cap in the GlobalState account
        let global_state = &mut ctx.accounts.global_state;
    
        // Validate authority (only the admin can update this)
        if ctx.accounts.authority.key() != global_state.authority {
            return Err(ErrorCode::Unauthorized.into());
        }
    
        // Update the market cap
        global_state.current_market_cap = market_cap;
    
        // Emit the MarketCapUpdated event
        emit!(MarketCapUpdated {
            authority: ctx.accounts.authority.key(),
            market_cap,
        });
    
        // Log the update
        msg!(
            "Market cap updated by authority: {:?} to {}",
            ctx.accounts.authority.key(),
            market_cap
        );
    
        Ok(())
    } 

    pub fn register_user_on_project_wallet_transfer(
        ctx: Context<RegisterUserOnTransfer>,
        user_wallet: Pubkey, // The wallet address of the user
        locked_amount: u64,  // The locked token amount to track
    ) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // STEP 1: Derive the User PDA
        let (user_pda, bump) = Pubkey::find_program_address(
            &[b"user_state", user_wallet.as_ref()], // PDA seeds
            ctx.program_id,
        );

        // Ensure that the provided PDA matches the derived address
        if ctx.accounts.user_state.key() != user_pda {
            return Err(ErrorCode::UserNotFound.into());
        }

        // STEP 2: Deserialize PDA or Initialize If Necessary
        if ctx.accounts.user_state.data_is_empty() {
            // If the PDA is empty, initialize it
            let lamports = Rent::get()?.minimum_balance(MilestoneUnlockAccount::LEN);

            let create_instruction = solana_program::system_instruction::create_account(
                &ctx.accounts.authority.key(), // Payer
                &user_pda,                     // New PDA
                lamports,                      // Rent exemption
                MilestoneUnlockAccount::LEN as u64, // Space required
                ctx.program_id,                // PDA owner = this program
            );

            solana_program::program::invoke_signed(
                &create_instruction,
                &[
                    ctx.accounts.system_program.to_account_info(),
                    ctx.accounts.user_state.to_account_info(),
                    ctx.accounts.authority.to_account_info(),
                ],
                &[&[b"user_state", user_wallet.as_ref(), &[bump]]], // PDA seeds and bump
            )?;

            // Initialize state manually now that PDA is created
            let mut data = ctx.accounts.user_state.try_borrow_mut_data()?;
            let milestone_state = MilestoneUnlockAccount {
                wallet: user_wallet,
                ata: get_associated_token_address(&user_wallet, &global_state.token_mint),
                total_locked_tokens: locked_amount,
                unlocked_tokens: 0,
                last_unlocked_milestone: 0,
            };

            // Serialize the initialized state into the PDA's account
            milestone_state.serialize(&mut &mut data[..])?;

            msg!(
                "Initialized and registered new PDA for user {:?} with locked tokens: {}",
                user_wallet,
                locked_amount
            );

            global_state.user_count += 1;
        } else {
            // PDA exists: Deserialize the data
            let mut data = ctx.accounts.user_state.try_borrow_mut_data()?;
            let mut milestone_account =
                MilestoneUnlockAccount::try_from_slice(&data).map_err(|_| ErrorCode::DeserializationFailed)?;

            // Update the existing PDA data
            milestone_account.total_locked_tokens = milestone_account
                .total_locked_tokens
                .checked_add(locked_amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;

            // Serialize the updated data back into the PDA's account
            milestone_account.serialize(&mut &mut data[..])?;

            msg!(
                "Updated user PDA: {:?} with additional locked tokens: {}",
                user_wallet,
                locked_amount
            );
        }

        // STEP 3: Emit Event
        emit!(UserRegistered {
            wallet: user_wallet,
            locked_tokens: locked_amount,
            ata: get_associated_token_address(&user_wallet, &global_state.token_mint),
        });

        Ok(())
    }
}

#[account]
#[derive(Default)] // Add default implementation for easier initialization
pub struct GlobalState {
    pub authority: Pubkey,                        // Admin authority (32 bytes)
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
    pub auto_sell_triggered: bool,
}

// Constants for Milestone size
const MILESTONE_SIZE: usize = 8 + 1; // u64 (8 bytes) + u8 (1 byte)

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
        + 1;
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
pub struct InitializeProgram<'info> {
    #[account(init, payer = authority, space = 8 + GlobalState::LEN)]
    pub global_state: Account<'info, GlobalState>,

    pub token_mint: Account<'info, Mint>, // Correct type for SPL Token Mint
    /// CHECK: This is a standard wallet account, and the program will verify its usage
    pub burn_wallet: AccountInfo<'info>, // Standard wallet (not SPL Token Account)
    /// CHECK: This is a standard wallet account, and the program will verify its usage
    pub marketing_wallet: AccountInfo<'info>, // Standard wallet
    /// CHECK: This is a standard wallet account, and the program will verify its usage
    pub project_wallet: AccountInfo<'info>, // Standard wallet

    #[account(mut)]
    pub authority: Signer<'info>, // Admin authority

    pub system_program: Program<'info, System>, // System Program
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
    /// CHECK: Raydium or Orca destination wallet for liquidity (e.g., a USDC SPL token account)
    #[account(mut)]
    pub liquidity_wallet: AccountInfo<'info>, // Wallet to hold liquidity
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>,
    pub project_wallet_authority: Signer<'info>,
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

    pub authority: Signer<'info>, // Admin authority to sign the transaction
}

#[derive(Accounts)]
pub struct UpdateMarketCap<'info> {
    #[account(
        mut,                                      // Global state is being updated
        has_one = authority                       // Ensure authority matches the stored authority in global state
    )]
    pub global_state: Account<'info, GlobalState>, // Global state account
    pub authority: Signer<'info>,                 // Signer (admin authority)
}

#[derive(Accounts)]
pub struct RegisterUserOnTransfer<'info> {
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>, // Global state account that stores global data
    #[account(mut)]
    pub project_wallet: Account<'info, TokenAccount>, // Project wallet receiving locked tokens
    #[account(mut)]
    pub authority: Signer<'info>, // The signer who calls this transaction (payer for accounts)
    pub rent: Sysvar<'info, Rent>, // Rent sysvar
    pub system_program: Program<'info, System>,  // System program (for creating PDAs)
    pub token_program: Program<'info, Token>, // SPL Token Program for token transfers
    /// CHECK: Associated Token Program
    pub associated_token_program: AccountInfo<'info>,
    #[account(mut)] // User state PDA manually validated inside the function
    pub user_state: AccountInfo<'info>, // User-specific PDA account
}



fn get_token_balance(token_account: AccountInfo) -> Result<u64> {
    // Use the spl_token::state::Account to access the token balance
    let data = spl_token::state::Account::unpack(&token_account.data.borrow()).map_err(|_| ErrorCode::TokenAccountCreationFailed)?;
    Ok(data.amount)
}

// fn trigger_auto_sell(ctx: &Context<UnlockTokens>) -> Result<()> {
//     let global_state = &ctx.accounts.global_state;
//     let project_wallet = &ctx.accounts.project_wallet;

//     // Fetch Project Wallet balance
//     let project_wallet_balance = get_token_balance(ctx.accounts.project_wallet.to_account_info())?;
//     if project_wallet_balance == 0 {
//         return Err(ErrorCode::InsufficientFundsForAutoSell.into());
//     }

//     // Calculate 25% of the Project Wallet balance for auto-sell
//     let sell_amount = project_wallet_balance
//         .checked_div(4) // 25% of tokens
//         .ok_or(ErrorCode::ArithmeticOverflow)?;

//     // Ensure the sell amount is greater than 0
//     if sell_amount == 0 {
//         return Err(ErrorCode::InsufficientSellAmount.into());
//     }

//     msg!("Initiating auto-sell of {} tokens for liquidity...", sell_amount);

//     // --- Add Raydium/Orca Swap CPI Logic Here ---
//     // For example: You'd interact with Raydium's AMM program to perform the token swap.
//     // Example: Transfer tokens from Project Wallet and swap via Raydium.
//     // (Details provided earlier on how to use CPI with Raydium's AMM).

//     // Alternatively, you can emit an event for off-chain bots to execute Raydium swap:
//     emit!(AutoSellTriggered {
//         sell_amount,
//         wallet: project_wallet.key(), // Project Wallet performing the transaction
//         destination: ctx.accounts.liquidity_wallet.key(), // Liquidity destination wallet
//     });

//     msg!(
//         "Auto-sell triggered: {} tokens sold from {:?}",
//         sell_amount,
//         project_wallet.key()
//     );

//     Ok(())
// }

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
    #[msg("User's accounts not enough in remaining accounts.")]
    AccountNotEnough,
    #[msg("Token transfer failed")]
    TokenTransferFailed,
    #[msg("Next milestone not reached yet")]
    MiletoneNotReached,
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Invalid burn wallet account")]
    InvalidBurnWallet,
    #[msg("Insufficient funds for auto-sell")]
    InsufficientFundsForAutoSell,
    #[msg("Insufficient sell amount for auto-sell")]
    InsufficientSellAmount,
    #[msg("Invalid marketing wallet account")]
    InvalidMarketingWallet,
    #[msg("Invalid project wallet account")]
    InvalidProjectWallet,
    #[msg("Deserialization failed")]
    DeserializationFailed,
}

#[event]
pub struct UserRegistered {
    pub wallet: Pubkey,
    pub locked_tokens: u64,
    pub ata: Pubkey,
}

#[event]
pub struct MilestoneProcessed {
    pub wallet: Pubkey,
    pub unlocked_tokens: u64,
    pub milestone_index: u8,
}

#[event]
pub struct MarketCapUpdated {
    pub authority: Pubkey,    // Public key of the authority who triggered the update
    pub market_cap: u64,      // The new market cap value
}

#[event]
pub struct AutoSellTriggered {
    pub sell_amount: u64,              // Number of tokens sold
    pub wallet: Pubkey,                // Project Wallet public key
    pub destination: Pubkey,           // Liquidity destination (e.g., USDC wallet)
}