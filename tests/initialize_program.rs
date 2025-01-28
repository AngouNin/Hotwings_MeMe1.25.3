#[cfg(test)]
mod tests {
    use super::*; // Import your program and modules
    use anchor_lang::prelude::*;
    use anchor_lang::AccountDeserialize;
    use solana_program_test::*;
    use solana_sdk::{
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use hotwings::*; // Replace with the name of your program

    #[tokio::test]
    async fn test_initialize_program() {
        // Create a program test environment
        let mut program_test = ProgramTest::new(
            "hotwings", // Your program's name in Cargo.toml
            id("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow"),  // Program ID declared in `declare_id!`
            processor!(hotwings::entry), // Your program entry point
        );

        // Add accounts and data needed for the test
        let global_state = Keypair::new();
        let token_mint = Keypair::new();
        let authority = Keypair::new();

        // Start the test environment
        let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

        // Create the `global_state` account with appropriate space
        let global_state_rent = banks_client
            .get_rent()
            .await
            .unwrap()
            .minimum_balance(GlobalState::LEN);
        let create_global_state = solana_sdk::system_instruction::create_account(
            &payer.pubkey(),
            &global_state.pubkey(),
            global_state_rent,
            GlobalState::LEN as u64,
            &id(), // Program ID
        );

        // Create the transaction
        let mut transaction = Transaction::new_with_payer(
            &[
                create_global_state, // Instruction to create the global_state account
                Instruction {
                    program_id: id(),
                    accounts: vec![
                        AccountMeta::new(global_state.pubkey(), false),
                        AccountMeta::new_readonly(token_mint.pubkey(), false),
                        AccountMeta::new_readonly(authority.pubkey(), true),
                        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                    ],
                    data: anchor_lang::InstructionData::data(
                        &aaa::instruction::InitializeProgram {},
                    ), // Call your program's instruction
                },
            ],
            Some(&payer.pubkey()),
        );

        transaction.sign(&[&payer, &global_state, &authority], recent_blockhash);

        // Process the transaction
        banks_client.process_transaction(transaction).await.unwrap();

        // Fetch the account and verify the results
        let global_state_account = banks_client
            .get_account(global_state.pubkey())
            .await
            .expect("Global state account not found")
            .expect("Failed to fetch global state account");

        let global_state_data = GlobalState::try_deserialize(&mut global_state_account.data.as_ref())
            .expect("Failed to deserialize global state");

        assert_eq!(global_state_data.authority, authority.pubkey());
        assert_eq!(global_state_data.token_mint, token_mint.pubkey());
    }
}
