import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { SealevelCoreAdapter } from "@hyperlane-xyz/sdk";
import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  Keypair,
  sendAndConfirmTransaction,
  SystemProgram,
} from "@solana/web3.js";
import * as borsh from "borsh";
import { Buffer } from "buffer";

// Hyperlane Register program ID
const REGISTER_PROGRAM_ID = new PublicKey(
  "4KdqVph6eMnS2omUBLBH2u4G6wwqxG5hzesZpsFcSWod",
);

// Mailbox program ID
const MAILBOX_PROGRAM_ID = new PublicKey(
  "75HBBLae3ddeneJVrZeyrDfv6vb7SMC3aCpBucSXS5aR",
);

export const SEALEVEL_SPL_NOOP_ADDRESS =
  "noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV";

// Define the RegisterMessage structure (matches your Rust struct)
class RegisterMessage {
  constructor(destination, embedded_user) {
    this.destination = destination;
    this.embedded_user = embedded_user;
  }
}

// Define the instruction enum (matches your Rust enum)
class HyperlaneRegisterInstruction {
  constructor(register_message) {
    this.instruction = 0; // SendRegister variant
    this.register_message = register_message;
  }
}

// Borsh schema for serialization
const SCHEMA = new Map([
  [
    RegisterMessage,
    {
      kind: "struct",
      fields: [
        ["destination", "u32"],
        ["embedded_user", [32]], // 32-byte array for Pubkey
      ],
    },
  ],
  [
    HyperlaneRegisterInstruction,
    {
      kind: "struct",
      fields: [
        ["instruction", "u8"],
        ["register_message", RegisterMessage],
      ],
    },
  ],
]);

export async function executeRegisterProgram(
  connection,
  payer,
  destination,
  embeddedUser,
) {
  try {
    // Create the register message
    const registerMessage = new RegisterMessage(destination, embeddedUser);
    const instruction = new HyperlaneRegisterInstruction(registerMessage);

    console.log("instruction", instruction);

    // Serialize the instruction data
    const instructionData = borsh.serialize(SCHEMA, instruction);

    const randomWallet = Keypair.generate();
    const keys = [
      {
        pubkey: SealevelCoreAdapter.deriveMailboxOutboxPda(MAILBOX_PROGRAM_ID),
        isSigner: false,
        isWritable: true,
      },
      {
        pubkey:
          SealevelCoreAdapter.deriveMailboxDispatchAuthorityPda(
            REGISTER_PROGRAM_ID,
          ),
        isSigner: false,
        isWritable: false,
      },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      {
        pubkey: new PublicKey(SEALEVEL_SPL_NOOP_ADDRESS),
        isSigner: false,
        isWritable: false,
      },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      {
        pubkey: randomWallet.publicKey,
        isSigner: true,
        isWritable: false,
      },
      {
        pubkey: SealevelCoreAdapter.deriveMailboxDispatchedMessagePda(
          MAILBOX_PROGRAM_ID,
          randomWallet.publicKey,
        ),
        isSigner: false,
        isWritable: true,
      },
    ];
    console.log(keys);

    // Create the transaction instruction
    const registerInstruction = new TransactionInstruction({
      keys,
      programId: REGISTER_PROGRAM_ID,
      data: Buffer.from(instructionData),
    });

    // Create and send transaction
    const transaction = new Transaction().add(registerInstruction);

    console.log("Sending transaction...");
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [payer, randomWallet],
      {
        commitment: "confirmed",
      },
    );

    console.log("Transaction confirmed!");
    console.log("Signature:", signature);

    // Get transaction details to see logs
    const txDetails = await connection.getTransaction(signature, {
      commitment: "confirmed",
    });

    if (txDetails?.meta?.logMessages) {
      console.log("\n=== Transaction Logs ===");
      txDetails.meta.logMessages.forEach((log, index) => {
        console.log(`${index + 1}: ${log}`);
      });
    }

    // Look for the message ID in the logs
    const registerLog = txDetails?.meta?.logMessages?.find((log) =>
      log.includes("register "),
    );

    if (registerLog) {
      const messageId = registerLog.split("register ")[1];
      console.log("\n=== Message ID ===");
      console.log("Message ID:", messageId);
    }

    return {
      signature,
      messageId: registerLog?.split("register ")[1] || null,
      success: true,
    };
  } catch (error) {
    console.error("Error executing register program:", error);
    return {
      signature: null,
      messageId: null,
      success: false,
      error: error.message,
    };
  }
}

const PAYER_KEYPAIR = Keypair.fromSecretKey(
  new Uint8Array([
    138, 41, 218, 226, 33, 154, 255, 107, 6, 18, 194, 95, 95, 1, 209, 64, 94,
    117, 217, 1, 80, 74, 103, 30, 127, 82, 51, 44, 238, 236, 201, 59, 78, 87,
    203, 109, 253, 116, 49, 9, 206, 184, 176, 44, 135, 138, 250, 209, 21, 11, 0,
    235, 242, 112, 20, 221, 216, 249, 106, 95, 30, 156, 45, 136,
  ]),
);
const destination = 1; // Destination domain
const embeddedUser = new PublicKey("11111111111111111111111111111113"); // Example embedded user

async function doTransaction() {
  const connection = new Connection(
    "https://multi-wispy-sheet.solana-testnet.quiknode.pro/9bc33e3047c4a6c86c9254bead094eae0766d076",
    "confirmed",
  );
  executeRegisterProgram(
    connection,
    PAYER_KEYPAIR,
    destination,
    embeddedUser.toBytes(),
  );
}

doTransaction().then(console.log).catch(console.log);
