import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Hotwings } from "../target/types/hotwings";
import * as spl from "@solana/spl-token";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("hotwings_initialize", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);

  const program = anchor.workspace.Hotwings as Program<Hotwings>;
  let globalStateAccount: PublicKey;

  let tokenMint: PublicKey; // Placeholder for SPL Token Mint
  const burnWallet = anchor.web3.Keypair.generate().publicKey;
  const marketingWallet = anchor.web3.Keypair.generate().publicKey;
  const projectWallet = anchor.web3.Keypair.generate().publicKey;

  const raydiumProgramId = new PublicKey("Raydium1111111111111111111111111111111111");

  async function createMint(provider: anchor.AnchorProvider): Promise<PublicKey> {
    const mint = anchor.web3.Keypair.generate();
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(spl.MINT_SIZE);

    const mintTx = new anchor.web3.Transaction();
    mintTx.add(
      anchor.web3.SystemProgram.createAccount({
        fromPubkey: provider.wallet.publicKey,
        newAccountPubkey: mint.publicKey,
        space: spl.MINT_SIZE,
        programId: spl.TOKEN_PROGRAM_ID,
        lamports: lamports,
      }),
      spl.createInitializeMintInstruction(
        mint.publicKey,
        9,
        provider.wallet.publicKey,
        provider.wallet.publicKey
      )
    );

    await provider.sendAndConfirm(mintTx, [mint]);
    return mint.publicKey;
  }

  before(async () => {
    // Create the token mint
    tokenMint = await createMint(provider);

    // Airdrop to other accounts
    for (const wallet of [burnWallet, marketingWallet, projectWallet]) {
      const tx = await provider.connection.requestAirdrop(wallet, 10 * anchor.web3.LAMPORTS_PER_SOL);
      await provider.connection.confirmTransaction(tx);
    }

    console.log("Token Mint:", tokenMint.toBase58());
    console.log("Wallets funded.");
  });

  it("Successfully initializes the program", async () => {
    // Derive GlobalState PDA
    const [globalStatePDA, _bump] = await PublicKey.findProgramAddress(
      [Buffer.from("global_state")],
      program.programId
    );

    globalStateAccount = globalStatePDA;

    try {
      // Call initializeProgram
      await program.methods
        .initializeProgram(raydiumProgramId)
        .accounts({
          globalState: globalStateAccount, // Global state PDA
          tokenMint: tokenMint,            // SPL token mint
          burnWallet: burnWallet,          // Burn wallet (funded)
          marketingWallet: marketingWallet,// Marketing wallet (funded)
          projectWallet: projectWallet,    // Project wallet (funded)
          authority: provider.wallet.publicKey, // Admin authority
          systemProgram: anchor.web3.SystemProgram.programId, // <-- This is now valid
        })
        .rpc();

      console.log("Program initialized successfully!");

      // Fetch and verify state
      const globalState = await program.account.globalState.fetch(globalStateAccount);

      expect(globalState.authority.toBase58()).to.equal(provider.wallet.publicKey.toBase58());
      expect(globalState.tokenMint.toBase58()).to.equal(tokenMint.toBase58());
      expect(globalState.burnWallet.toBase58()).to.equal(burnWallet.toBase58());
      expect(globalState.marketingWallet.toBase58()).to.equal(marketingWallet.toBase58());
      expect(globalState.projectWallet.toBase58()).to.equal(projectWallet.toBase58());
      expect(globalState.raydiumProgramId.toBase58()).to.equal(raydiumProgramId.toBase58());
    } catch (err) {
      console.error("Initialization failed", err);
      throw err;
    }
  });
});