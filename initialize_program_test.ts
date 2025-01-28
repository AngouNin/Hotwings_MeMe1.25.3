import * as anchor from "@project-serum/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as dotenv from "dotenv";
import idl from './target/idl/hotwings.json';
import { Program } from '@project-serum/anchor';
import type { Hotwings } from "./target/types/hotwings.ts";


// Load environment variables from .env
dotenv.config();
const anchorProviderUrl = process.env.ANCHOR_PROVIDER_URL;
console.log("ANCHOR_PROVIDER_URL:", anchorProviderUrl);
console.log("ANCHOR_WALLET:", process.env.ANCHOR_WALLET);
console.log("PROGRAM_ID:", process.env.PROGRAM_ID);
console.log("TOKEN_MINT:", process.env.TOKEN_MINT);


// Set up provider using the loaded environment variables
const provider = anchor.AnchorProvider.env();
anchor.setProvider(provider);

if (!process.env.PROGRAM_ID) {
  throw new Error("PROGRAM_ID is not defined in the environment variables");
}
const programId = new anchor.web3.PublicKey(process.env.PROGRAM_ID); // PROGRAM_ID from .env

const program = new anchor.Program(idl as unknown as Hotwings, programId, provider);
console.log("===========>pass");

async function main() {
  const admin = provider.wallet;

  // Token mint from environment variables
  if (!process.env.TOKEN_MINT) {
    throw new Error("TOKEN_MINT is not defined in the environment variables");
  }
  const tokenMint = new PublicKey(process.env.TOKEN_MINT);

  // Derive PDA for global state
  const [globalStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-state")],
    program.programId
  );

  console.log("Global State PDA:", globalStatePDA.toString());

  try {
    const tx = await program.methods
      .initializeProgram()
      .accounts({
        globalState: globalStatePDA,
        tokenMint: tokenMint,
        authority: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log("Transaction successful, TX ID:", tx);
  } catch (err) {
    console.error("Transaction failed:", err);
  }
}

main().catch((err) => console.error(err));
