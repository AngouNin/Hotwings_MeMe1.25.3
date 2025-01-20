use anchor_lang::prelude::*;

#[event]
pub struct UserRegistered {
    pub wallet: Pubkey,
    pub locked_tokens: u64,
    pub ata: Pubkey,
}

#[event]
pub struct MarketCapUpdated {
    ///CHECK: The authority who triggered the update
    pub authority: Pubkey,    // Public key of the authority who triggered the update
    pub market_cap: u64,      // The new market cap value
}


#[event]
pub struct MilestoneProcessed {
    pub wallet: Pubkey,
    pub unlocked_tokens: u64,
    pub milestone_index: u8,
    pub tax: u64,
    pub burn_tax: u64,
    pub marketing_tax: u64,
}