use anchor_lang::prelude::*;

declare_id!("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow");

#[program]
pub mod hotwing_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
