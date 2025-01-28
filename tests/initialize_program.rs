use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use anchor_client::solana_sdk::{
    signature::{Keypair, Signer},
    system_program,
};
use solana_program::pubkey::Pubkey;
use anchor_client::Client;

#[tokio::test]
async fn test_initialize_program() {
    // Configure the Anchor client to connect to the devnet.
    let url = "https://api.devnet.solana.com";
    let wallet = anchor_client::solana_sdk::signer::keypair::read_keypair_file(
        std::env::var("ANCHOR_WALLET").unwrap(),
    )
    .unwrap();
    let client = Client::new_with_options(url, wallet, CommitmentConfig::processed());

    // Define the program and payer.
    let program_id: Pubkey = "L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow".parse().unwrap();
    let program = client.program(program_id);

    // Define the accounts and seeds for initialization.
    let global_state = Keypair::new();
    let burn_wallet = Keypair::new();
    let marketing_wallet = Keypair::new();
    let project_wallet = Keypair::new();
    let token_mint = Keypair::new();

    // Airdrop SOL to accounts for testing.
    for keypair in [&burn_wallet, &marketing_wallet, &project_wallet, &token_mint] {
        client
            .request_airdrop(&keypair.pubkey(), 2_000_000_000)
            .await
            .unwrap();
    }

    // Call the initialize_program function.
    let tx = program
        .request()
        .accounts(hotwings::accounts::InitializeProgram {
            global_state: global_state.pubkey(),
            burn_wallet: burn_wallet.pubkey(),
            marketing_wallet: marketing_wallet.pubkey(),
            project_wallet: project_wallet.pubkey(),
            token_mint: token_mint.pubkey(),
            system_program: system_program::ID,
        })
        .args(hotwings::instruction::InitializeProgram {
            raydium_program_id: Pubkey::new_unique(),
        })
        .signer(&global_state)
        .signer(&burn_wallet)
        .signer(&marketing_wallet)
        .signer(&project_wallet)
        .send();

    assert!(tx.is_ok(), "Transaction failed: {:?}", tx.err());
    println!("Test passed: Program initialized successfully.");
}
