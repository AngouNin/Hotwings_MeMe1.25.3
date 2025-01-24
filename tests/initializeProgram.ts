import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Hotwings} from "../target/types/hotwings";
import { assert } from "chai";
import idl from "../target/idl/hotwings.json";      // Raw IDL JSON

describe("Test initialize_program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const programId = new anchor.web3.PublicKey("L1dCurNdHKSmpRHFKGcaNf64qzExvCMGuZbU3uun6ow");
  const program = new Program(idl as unknown as anchor.Idl, programId, provider); // Use the raw IDL for the program instance

  it("Initializes the program with correct state", async () => {
    // Create keypairs for authority and wallets
    const authority = provider.wallet.publicKey;
    const burnWallet = anchor.web3.Keypair.generate();
    const marketingWallet = anchor.web3.Keypair.generate();
    const projectWallet = anchor.web3.Keypair.generate();
    const raydiumProgramId = new anchor.web3.PublicKey("Raydium11111111111111111111111111111111111111");

    // Generate an account for global state
    const [globalStatePda, _] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("global-state")],
      program.programId
    );

    const tokenMint = anchor.web3.Keypair.generate(); // Random mint for testing

    // Airdrop lamports for testing accounts
    const wallets = [burnWallet, marketingWallet, projectWallet];
    for (let wallet of wallets) {
      const tx = await provider.connection.requestAirdrop(wallet.publicKey, 1e9); // 1 SOL
      await provider.connection.confirmTransaction(tx);
    }

    // Call the initialize_program function
    const tx = await program.methods
      .initializeProgram(raydiumProgramId)
      .accounts({
        authority,
        globalState: globalStatePda,
        tokenMint: tokenMint.publicKey,
        burnWallet: burnWallet.publicKey,
        marketingWallet: marketingWallet.publicKey,
        projectWallet: projectWallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([])
      .rpc();

    console.log("Transaction signature:", tx);

    // Fetch the initialized global state
    const globalState = await program.account.globalState.fetch(globalStatePda) as any;

    // Validate the initialized state
    assert.strictEqual(globalState.authority.toBase58(), authority.toBase58());
    assert.strictEqual(globalState.tokenMint.toBase58(), tokenMint.publicKey.toBase58());
    assert.strictEqual(globalState.burnWallet.toBase58(), burnWallet.publicKey.toBase58());
    assert.strictEqual(globalState.marketingWallet.toBase58(), marketingWallet.publicKey.toBase58());
    assert.strictEqual(globalState.projectWallet.toBase58(), projectWallet.publicKey.toBase58());
    assert.strictEqual(globalState.raydiumProgramId.toBase58(), raydiumProgramId.toBase58());
    assert.strictEqual(globalState.currentMarketCap.toString(), "0");
    assert.strictEqual(globalState.userCount.toString(), "0");

    console.log("Global state initialized successfully!");
  });
});