use anchor_lang::prelude::*;

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
    #[msg("Invalid marketing wallet account")]
    InvalidMarketingWallet,
    #[msg("Invalid project wallet account")]
    InvalidProjectWallet,
    #[msg("Deserialization failed")]
    DeserializationFailed,
    #[msg("Serialization failed")]
    SerializationFailed,
    #[msg("Invalid market cap value")]
    InvalidMarketCapValue, 
    #[msg("Account Not Found")]
    AccountNotFound,
    #[msg("Liquidity pool not set.")]
    LiquidityPoolNotSet,
    #[msg("Mileston has finished.")]
    MiletoneCompleted,
}