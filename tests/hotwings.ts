import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { Hotwings } from "../target/types/hotwings";

describe("initialize_program", () => {
  // Configure the client to use the local cluster or devnet.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Hotwings as Program<Hotwings>;

  it("Initializes the program successfully", async () => {
    // Accounts
    const provider = anchor.getProvider();
    const authority = provider.wallet;
    const tokenMint = anchor.web3.Keypair.generate();
    const burnWallet = anchor.web3.Keypair.generate();
    const marketingWallet = anchor.web3.Keypair.generate();
    const projectWallet = anchor.web3.Keypair.generate();

    // Global state PDA
    const [globalStatePda, globalStateBump] = await PublicKey.findProgramAddress(
      [Buffer.from("global_state")],
      program.programId
    );

    // Initialize program
    const tx = await program.methods
      .initializeProgram(new PublicKey("RaydiumProgramID")) // Replace with actual Raydium program ID
      .accounts({
        globalState: globalStatePda,
        tokenMint: tokenMint.publicKey,
        burnWallet: burnWallet.publicKey,
        marketingWallet: marketingWallet.publicKey,
        projectWallet: projectWallet.publicKey,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([tokenMint, burnWallet, marketingWallet, projectWallet])
      .rpc();

    console.log("Transaction Signature:", tx);

    // Fetch the global state to verify initialization
    const globalState = await program.account.globalState.fetch(globalStatePda);

    // Assertions
    expect(globalState.tokenMint.toBase58()).to.equal(tokenMint.publicKey.toBase58());
    expect(globalState.burnWallet.toBase58()).to.equal(burnWallet.publicKey.toBase58());
    expect(globalState.marketingWallet.toBase58()).to.equal(marketingWallet.publicKey.toBase58());
    expect(globalState.projectWallet.toBase58()).to.equal(projectWallet.publicKey.toBase58());
    expect(globalState.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    console.log("Initialization successful and verified!");
  });
});
