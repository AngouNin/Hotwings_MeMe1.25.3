use anchor_lang::prelude::*;
use spl_token::solana_program::program_pack::Pack;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    msg,
    instruction::Instruction,
}; 
use solana_program::program::invoke;
use spl_token_2022::id as token_2022_program_id;

use crate::consts::*;
use crate::structs::*;
use crate::errors::ErrorCode;
use crate::events::*;

pub mod consts;
pub mod structs;
pub mod errors;
pub mod events;

declare_id!("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow");

/// Program module
#[program]
pub mod hotwing_contract {
    use anchor_spl::{associated_token::{get_associated_token_address, spl_associated_token_account}, token};
    use super::*;

    /// Initialize the program with milestones and setup global state
    pub fn initialize_program(ctx: Context<InitializeProgram>, raydium_program_id: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // Runtime validations for wallets
        let burn_wallet_account = &ctx.accounts.burn_wallet;
        let marketing_wallet_account = &ctx.accounts.marketing_wallet;
        let project_wallet_account = &ctx.accounts.project_wallet;
        // Ensure burn_wallet is not a PDA and is an externally owned account
        require!(
            burn_wallet_account.lamports() > 0 
            && burn_wallet_account.owner == &solana_program::system_program::ID,
            ErrorCode::InvalidBurnWallet
        );

        // Ensure marketing_wallet is not a PDA and is an externally owned account
        require!(
            marketing_wallet_account.lamports() > 0 
            && marketing_wallet_account.owner == &solana_program::system_program::ID,
            ErrorCode::InvalidMarketingWallet
        );

        // Ensure project_wallet is not a PDA and is an externally owned account
        require!(
            project_wallet_account.lamports() > 0 
            && project_wallet_account.owner == &solana_program::system_program::ID,
            ErrorCode::InvalidProjectWallet
        );
        // Ensure burn_wallet is a valid writable account 
        if !burn_wallet_account.is_writable {
            return Err(ErrorCode::InvalidBurnWallet.into());
            
        } 
        if !marketing_wallet_account.is_writable {
            return Err(ErrorCode::InvalidMarketingWallet.into());
        } 
        if !project_wallet_account.is_writable {
            return Err(ErrorCode::InvalidProjectWallet.into());
        }

        global_state.token_mint = ctx.accounts.token_mint.key(); 
        global_state.burn_wallet = ctx.accounts.burn_wallet.key();
        global_state.marketing_wallet = ctx.accounts.marketing_wallet.key();
        global_state.project_wallet = ctx.accounts.project_wallet.key(); 
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

        global_state.raydium_program_id = raydium_program_id;
        msg!("Global state initialized successfully with Raydium program ID: {:?}", raydium_program_id);

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
    pub fn process_milestones<'info>(
        ctx: Context<'_, '_, '_, 'info, UnlockTokens<'info>>,
        market_cap: u64,
    ) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Update global market cap
        global_state.current_market_cap = market_cap;
    
        // Check if three-month full unlocking conditions are met
        let current_timestamp = Clock::get()?.unix_timestamp;
        if current_timestamp >= global_state.three_month_unlock_date || market_cap >= 2_500_000 {
            msg!("Full unlocking allowed: Three months passed or max market cap reached.");
            global_state.unlock_complete = true;
        }
    
        // Iterate milestones to find the applicable range
        let mut milestone_idx = global_state.current_milestone as usize;
        while milestone_idx < MAX_MILESTONES
            && market_cap >= global_state.milestones[milestone_idx].market_cap
        {
            milestone_idx += 1;
        }
    
        if milestone_idx == global_state.current_milestone as usize {
            return Err(ErrorCode::MiletoneNotReached.into());
        }
    
        // Update the current milestone
        global_state.current_milestone = milestone_idx as u8;
    
        let milestone = &global_state.milestones[milestone_idx - 1]; // Unlock tokens for this milestone
    
        msg!(
            "Processing milestone {}: Unlock {}% (Current Market Cap: {})",
            milestone_idx - 1,
            milestone.unlock_percent,
            market_cap
        );
    
        let account_chunks: Vec<AccountInfo<'info>> = ctx.remaining_accounts.iter().cloned().collect::<Vec<AccountInfo<'info>>>();
        for account_chunk in account_chunks.chunks(2) {
            let mut account_iter = account_chunk.into_iter();
            let user_pda = account_iter.next().ok_or(ErrorCode::AccountNotEnough)?;
            let ata_account = account_iter.next().ok_or(ErrorCode::AccountNotEnough)?;
        
            // Deserialize user state
            let mut user_state: MilestoneUnlockAccount =
                AccountDeserialize::try_deserialize(&mut &**user_pda.try_borrow_mut_data()?)
                    .map_err(|_| ErrorCode::DeserializationFailed)?;
        
            // Calculate the unlockable tokens
            let tokens_to_unlock = user_state
                .total_locked_tokens
                .checked_mul(milestone.unlock_percent as u64)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / 100;
        
            let mut unlocked_tokens = tokens_to_unlock;
        
            if !global_state.unlock_complete {
                // Check for anti-whale exemptions
                if !global_state.exempted_wallets.contains(&user_state.wallet) {
                    let recipient_balance = get_token_balance(&ata_account)?;
                    let max_available_unlock = if recipient_balance + unlocked_tokens > MAX_HOLD_AMOUNT {
                        MAX_HOLD_AMOUNT.saturating_sub(recipient_balance)
                    } else {
                        unlocked_tokens
                    };
                    if max_available_unlock < unlocked_tokens {
                        msg!(
                            "Anti-whale restriction applied: Unlock limited for user {:?}",
                            user_state.wallet
                        );
                        unlocked_tokens = max_available_unlock;
                    }
                }
            }
        
            if unlocked_tokens == 0 {
                continue;
            }
        
            // Apply the tax deduction (1.5%)
            let tax = unlocked_tokens.checked_mul(15).ok_or(ErrorCode::ArithmeticOverflow)? / 1000;
            let post_tax_tokens = unlocked_tokens.checked_sub(tax).ok_or(ErrorCode::ArithmeticOverflow)?;
        
            // Split the tax between Burn Wallet and Marketing Wallet
            let tax_burn = tax.checked_div(2).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tax_marketing = tax.checked_sub(tax_burn).ok_or(ErrorCode::ArithmeticOverflow)?;
        
            // Transfer post-tax tokens to the user
            let cpi_transfer_to_user = token::Transfer {
                from: ctx.accounts.project_wallet.to_account_info(),
                to: ata_account.to_account_info(),
                authority: ctx.accounts.project_wallet_authority.to_account_info(),
            };
        
            token::transfer(
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_transfer_to_user),
                post_tax_tokens,
            )?;
        
            // Transfer tax to Burn Wallet
            let cpi_transfer_to_burn = token::Transfer {
                from: ctx.accounts.project_wallet.to_account_info(),
                to: ctx.accounts.burn_wallet.to_account_info(),
                authority: ctx.accounts.project_wallet_authority.to_account_info(),
            };
        
            token::transfer(
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_transfer_to_burn),
                tax_burn,
            )?;
        
            // Transfer tax to Marketing Wallet
            let cpi_transfer_to_marketing = token::Transfer {
                from: ctx.accounts.project_wallet.to_account_info(),
                to: ctx.accounts.marketing_wallet.to_account_info(),
                authority: ctx.accounts.project_wallet_authority.to_account_info(),
            };
        
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    cpi_transfer_to_marketing,
                ),
                tax_marketing,
            )?;
        
            // Update user state
            user_state.total_locked_tokens = user_state
                .total_locked_tokens
                .checked_sub(unlocked_tokens)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            user_state.unlocked_tokens = user_state
                .unlocked_tokens
                .checked_add(unlocked_tokens)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            user_state.last_unlocked_milestone = global_state.current_milestone;
        
            // Serialize updated state back into user PDA
            user_state
                .try_serialize(&mut *user_pda.try_borrow_mut_data()?)
                .map_err(|_| ErrorCode::SerializationFailed)?;
        
            // Emit milestone processed event
            emit!(MilestoneProcessed {
                wallet: user_state.wallet,
                unlocked_tokens: post_tax_tokens, // Post-tax tokens unlocked
                milestone_index: global_state.current_milestone,
                tax,                              // Total tax amount deducted
                burn_tax: tax_burn,               // Tax sent to Burn Wallet
                marketing_tax: tax_marketing,     // Tax sent to Marketing Wallet
            });
        
            msg!(
                "User {:?}: Unlocked {} tokens ({} taxed, {} to Burn Wallet, {} to Marketing Wallet)",
                user_state.wallet,
                post_tax_tokens,
                tax,
                tax_burn,
                tax_marketing
            );
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

    pub fn update_raydium_program_id(ctx: Context<UpdateRaydiumProgramId>, new_raydium_program_id: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
    
        // Ensure only the authority can execute this instruction
        if ctx.accounts.authority.key() != global_state.authority {
            return Err(ErrorCode::Unauthorized.into());
        }
    
        // Update the Raydium program ID
        global_state.raydium_program_id = new_raydium_program_id;
    
        msg!("Raydium program ID updated to: {:?}", new_raydium_program_id);
    
        Ok(())
    }

}

// <<<<<<<<<<<<<<< PLACE THIS OUTSIDE THE #[program] BLOCK >>>>>>>>>>>>>>>

/// Implement the `on_transfer` hook as a separate function.
/// Token-2022 will invoke it automatically on token transfer.
/// This function is NOT an Anchor instruction.
/// Function that serves as the on-transfer hook
pub fn on_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo], // Account list: source, destination, global state, etc.
    amount: u64,              // Number of tokens being transferred
) -> Result<()> {
    let account_iter = &mut accounts.iter();

    // (1) Extract necessary accounts from the account list
    let source_account = next_account_info(account_iter)?;           // Source account (sending tokens)
    let destination_account = next_account_info(account_iter)?;      // Destination account
    let global_state_account = next_account_info(account_iter)?;     // Global state account
    let project_wallet_account = next_account_info(account_iter)?;   // Project wallet to redirect tokens
    let mint_account = next_account_info(account_iter)?;             // Mint account

    // (2) Fetch and deserialize the GlobalState account
    let global_state: GlobalState = GlobalState::try_from_slice(&global_state_account.data.borrow())
        .map_err(|_| ProgramError::InvalidAccountData)?;

    // Ensure it's your program handling the on_transfer hook
    if program_id != &crate::id() {
        return Err(ProgramError::IncorrectProgramId.into());
    }

    msg!(
        "on_transfer: Source = {:?}, Destination = {:?}, Amount = {}",
        source_account.key,
        destination_account.key,
        amount
    );

    // (3) Detect source program for AMM/Raydium transactions
    if RaydiumTransactionHelper::is_raydium_transaction(
        source_account,
        global_state.raydium_program_id,
    ) {
        msg!("Raydium transaction detected. Redirecting tokens to project wallet.");

        
        // Transfer all tokens to the Project Wallet
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::id(),
            source_account.key,
            &mint_account.key(),
            project_wallet_account.key,
            source_account.key, // Must be signed by source authority
            &[],
            amount,
            9, // Decimal places (change if your token has decimals)
        );

        solana_program::program::invoke(
            &transfer_ix?,
            &[
                source_account.clone(),
                project_wallet_account.clone(),
                destination_account.clone(),
            ],
        )?;

        // Log post-transfer
        msg!("Tokens redirected to project wallet.");
    } else {
        msg!("Normal transfer logic executed.");
    }

    Ok(())
}

pub fn register_transfer_hook(ctx: Context<RegisterHook>) -> Result<()> {
    // Define the update_transfer_hook function
    fn update_transfer_hook(
        token_program_id: &Pubkey,
        mint: &Pubkey,
        hook_program_id: Option<&Pubkey>,
        authority: &Pubkey,
    ) -> std::result::Result<Instruction, ProgramError> {
        let accounts = vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new_readonly(*authority, true),
        ];

        let data = {
            let mut data = Vec::with_capacity(1 + 32 + 1);
            data.push(0); // Instruction identifier for update_transfer_hook
            data.extend_from_slice(mint.as_ref());
            data.push(hook_program_id.is_some() as u8);
            if let Some(hook_program_id) = hook_program_id {
                data.extend_from_slice(hook_program_id.as_ref());
            }
            data
        };

        Ok(Instruction {
            program_id: *token_program_id,
            accounts,
            data,
        })
    }

    // Program ID to set as the TransferHook handler (your program's ID)
    let transfer_hook_program_id = crate::id();

    // Construct the instruction to update the transfer hook
    let update_ix = update_transfer_hook(
        &token_2022_program_id(),                     // SPL Token-2022 program
        &ctx.accounts.token_mint.key(),              // Token-2022 mint address
        Some(&transfer_hook_program_id),             // Program to be set as the transfer-hook handler
        &ctx.accounts.authority.key(),               // Authority that manages the token mint
    )?;

    // Perform CPI (Cross-Program Invocation) to execute the instruction
    invoke(
        &update_ix,
        &[
            ctx.accounts.token_mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        ],
    )?;

    msg!("Transfer hook registered successfully!");
    Ok(())
}

fn get_token_balance(token_account: &AccountInfo) -> Result<u64> {
    // Use the spl_token::state::Account to access the token balance
    let data = spl_token::state::Account::unpack(&token_account.data.borrow()).map_err(|_| ErrorCode::TokenAccountCreationFailed)?;
    Ok(data.amount)
}