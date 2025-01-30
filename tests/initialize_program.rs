use anchor_lang::prelude::*;
use anchor_client::solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use solana_program_test::{processor, ProgramTest};
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::str::FromStr;
use hotwings::GlobalState;

#[tokio::test]
async fn test_initialize_program() {
    // Initialize Solana program test environment
    let program_id = Pubkey::from_str("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow").unwrap();
    let mut program_test = ProgramTest::new(
        "hotwings", // The name of your program (matches your `Cargo.toml`)
        program_id, // Program ID declared in your program
        processor!(hotwings::entry), // Use the program entrypoint
    );

    // Generate keypairs for the test
    let payer = Keypair::new(); // Payer for transaction fees
    let authority = Keypair::new(); // Admin authority
    let burn_wallet = Keypair::new();
    let marketing_wallet = Keypair::new();
    let project_wallet = Keypair::new();
    let token_mint = Keypair::new();
    let global_state = Keypair::new();

    // Add test accounts to the program environment
    program_test.add_account(
        burn_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000,
            owner: solana_program::system_program::ID,
            ..Account::default()
        },
    );
    program_test.add_account(
        marketing_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000,
            owner: solana_program::system_program::ID,
            ..Account::default()
        },
    );
    program_test.add_account(
        project_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000,
            owner: solana_program::system_program::ID,
            ..Account::default()
        },
    );

    // Start the Solana test environment
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Airdrop SOL to the payer for transaction fees
    let mut transaction = Transaction::new_with_payer(
        &[
            solana_program::system_instruction::transfer(
                &payer.pubkey(),
                &authority.pubkey(),
                10_000_000, // Send 0.01 SOL to the authority
            ),
        ],
        Some(&payer.pubkey()),
    );

    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Prepare the `initialize_program` instruction
    let raydium_program_id = Pubkey::from_str("11111111111111111111111111111111").unwrap(); // Dummy Raydium Program ID

    let ix = hotwings::instruction::initialize_program(
        program_id,
        hotwings::InitializeProgram {
            global_state: global_state.pubkey(),
            token_mint: token_mint.pubkey(),
            burn_wallet: burn_wallet.pubkey(),
            marketing_wallet: marketing_wallet.pubkey(),
            project_wallet: project_wallet.pubkey(),
            authority: authority.pubkey(),
        },
        raydium_program_id,
    );

    // Build and execute the transaction
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &global_state, &authority], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    // Verify the global state account
    let global_state_account = banks_client
        .get_account(global_state.pubkey())
        .await
        .unwrap()
        .expect("Global state account does not exist!");

    let global_state_data: GlobalState =
        anchor_lang::AccountDeserialize::try_deserialize(&mut global_state_account.data.as_ref())
            .unwrap();

    // Assert the values in the global state
    assert_eq!(global_state_data.authority, authority.pubkey());
    assert_eq!(global_state_data.token_mint, token_mint.pubkey());
    assert_eq!(global_state_data.burn_wallet, burn_wallet.pubkey());
    assert_eq!(global_state_data.marketing_wallet, marketing_wallet.pubkey());
    assert_eq!(global_state_data.project_wallet, project_wallet.pubkey());
    assert_eq!(global_state_data.raydium_program_id, raydium_program_id);

    println!("Test passed: initialize_program successfully set up the global state!");
}
