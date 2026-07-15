import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { NATIVE_MINT, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  type PublicKey,
  SystemProgram,
  type Transaction,
  type TransactionInstruction,
} from "@solana/web3.js";
import bs58 from "bs58";
import { ESCROW_PROGRAM_ID } from "./constants.js";
import { instructionCoder } from "./idl.js";
import {
  deriveCardVaultPda,
  deriveDuelPda,
  derivePaymentVaultPda,
  deriveResultCommitmentPda,
} from "./pdas.js";

const ACCOUNT_CONSTRAINTS = {
  cancel_unmatched: [
    ["creator", true, true],
    ["duel", false, true],
    ["payment_vault", false, true],
    ["creator_destination", false, true],
    ["payment_mint", false, false],
    ["token_program", false, false],
  ],
  close_card_vault: [
    ["caller", true, false],
    ["duel", false, false],
    ["card_vault", false, true],
    ["card_mint", false, false],
    ["rent_recipient", false, true],
    ["recovery_destination", false, true],
    ["token_program", false, false],
  ],
  close_payment_vault: [
    ["caller", true, false],
    ["duel", false, false],
    ["payment_vault", false, true],
    ["payment_mint", false, false],
    ["rent_recipient", false, true],
    ["excess_destination", false, true],
    ["token_program", false, false],
  ],
  deposit_card_asset: [
    ["depositor", true, true],
    ["duel", false, true],
    ["depositor_source", false, true],
    ["card_vault", false, true],
    ["card_mint", false, false],
    ["token_program", false, false],
    ["system_program", false, false],
  ],
  fund_duel: [
    ["player", true, true],
    ["duel", false, true],
    ["player_source", false, true],
    ["payment_vault", false, true],
    ["payment_mint", false, false],
    ["token_program", false, false],
  ],
  initialize_duel: [
    ["creator", true, true],
    ["duel", false, true],
    ["payment_vault", false, true],
    ["payment_mint", false, false],
    ["token_program", false, false],
    ["system_program", false, false],
  ],
  refund_expired_card: [
    ["caller", true, false],
    ["duel", false, true],
    ["card_vault", false, true],
    ["card_mint", false, false],
    ["destination", false, true],
    ["token_program", false, false],
  ],
  refund_expired_payment: [
    ["caller", true, false],
    ["duel", false, true],
    ["payment_vault", false, true],
    ["destination", false, true],
    ["payment_mint", false, false],
    ["token_program", false, false],
  ],
  settle_duel: [
    ["caller", true, false],
    ["duel", false, true],
    ["result_commitment", false, true],
    ["payment_vault", false, true],
    ["payment_mint", false, false],
    ["creator_payment_destination", false, true],
    ["opponent_payment_destination", false, true],
    ["fee_destination", false, true],
    ["creator_card_vault", false, true],
    ["creator_card_mint", false, false],
    ["creator_card_destination", false, true],
    ["opponent_card_vault", false, true],
    ["opponent_card_mint", false, false],
    ["opponent_card_destination", false, true],
    ["token_program", false, false],
  ],
  submit_result: [
    ["provider_signer", true, true],
    ["duel", false, true],
    ["result_commitment", false, true],
    ["system_program", false, false],
  ],
} as const;

export type EscrowInstructionName = keyof typeof ACCOUNT_CONSTRAINTS;

export interface MonitoredInstructionAccount {
  index: number;
  role: string;
  address: string;
  isSigner: boolean;
  isWritable: boolean;
}

export interface EscrowInstructionDescription {
  programId: string;
  name: EscrowInstructionName;
  accounts: MonitoredInstructionAccount[];
  dataBase58: string;
  dataBase58Sha256: string;
  dataBytesSha256: string;
}

export interface LegacyTransactionMessageBinding {
  encoding: "base64";
  messageBase64: string;
  messageSha256: string;
}

function isInstructionName(value: string): value is EscrowInstructionName {
  return value in ACCOUNT_CONSTRAINTS;
}

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function accountByRole(
  instruction: TransactionInstruction,
  name: EscrowInstructionName,
  role: string,
): PublicKey | undefined {
  const index = ACCOUNT_CONSTRAINTS[name].findIndex(
    ([candidate]) => candidate === role,
  );
  return index === -1 ? undefined : instruction.keys[index]?.pubkey;
}

function assertAddress(
  actual: PublicKey | undefined,
  expected: PublicKey,
  role: string,
): void {
  if (!actual?.equals(expected)) {
    throw new Error(`${role} must be ${expected.toBase58()}`);
  }
}

function assertCanonicalAddresses(
  instruction: TransactionInstruction,
  name: EscrowInstructionName,
): void {
  const tokenProgram = accountByRole(instruction, name, "token_program");
  if (tokenProgram) {
    assertAddress(tokenProgram, TOKEN_PROGRAM_ID, "token_program");
  }
  const systemProgram = accountByRole(instruction, name, "system_program");
  if (systemProgram) {
    assertAddress(systemProgram, SystemProgram.programId, "system_program");
  }
  const paymentMint = accountByRole(instruction, name, "payment_mint");
  if (paymentMint) {
    assertAddress(paymentMint, NATIVE_MINT, "payment_mint");
  }

  const duel = accountByRole(instruction, name, "duel");
  const paymentVault = accountByRole(instruction, name, "payment_vault");
  if (duel && paymentVault) {
    assertAddress(
      paymentVault,
      derivePaymentVaultPda(duel)[0],
      "payment_vault",
    );
  }

  if (name === "initialize_duel") {
    const creator = accountByRole(instruction, name, "creator");
    if (!creator || instruction.data.length < 16) {
      throw new Error("initialize_duel is missing creator or nonce data");
    }
    const nonce = new DataView(
      instruction.data.buffer,
      instruction.data.byteOffset + 8,
      8,
    ).getBigUint64(0, true);
    assertAddress(duel, deriveDuelPda(creator, nonce)[0], "duel");
  }

  if (
    name === "close_card_vault" ||
    name === "deposit_card_asset" ||
    name === "refund_expired_card"
  ) {
    const encodedRole = instruction.data[8];
    if (!duel || (encodedRole !== 0 && encodedRole !== 1)) {
      throw new Error(`${name} contains an invalid player role`);
    }
    if (name === "deposit_card_asset" && instruction.data[9] !== 0) {
      throw new Error(
        "deposit_card_asset contains an unsupported card asset standard",
      );
    }
    const role = encodedRole === 0 ? "creator" : "opponent";
    assertAddress(
      accountByRole(instruction, name, "card_vault"),
      deriveCardVaultPda(duel, role)[0],
      "card_vault",
    );
  }

  if (name === "settle_duel" && duel) {
    assertAddress(
      accountByRole(instruction, name, "creator_card_vault"),
      deriveCardVaultPda(duel, "creator")[0],
      "creator_card_vault",
    );
    assertAddress(
      accountByRole(instruction, name, "opponent_card_vault"),
      deriveCardVaultPda(duel, "opponent")[0],
      "opponent_card_vault",
    );
  }

  if (name === "submit_result") {
    const providerSigner = accountByRole(instruction, name, "provider_signer");
    if (!providerSigner || instruction.data.length < 202) {
      throw new Error("submit_result is missing its provider commitment data");
    }
    const providerRequestId = instruction.data.subarray(40, 72);
    assertAddress(
      accountByRole(instruction, name, "result_commitment"),
      deriveResultCommitmentPda(providerSigner, providerRequestId)[0],
      "result_commitment",
    );
    if (instruction.data[200] !== 0 || instruction.data[201] !== 0) {
      throw new Error(
        "submit_result contains an unsupported card asset standard",
      );
    }
  }
}

export function assertEscrowInstructionConstraints(
  instruction: TransactionInstruction,
): EscrowInstructionName {
  if (!instruction.programId.equals(ESCROW_PROGRAM_ID)) {
    throw new Error(
      `Unexpected escrow program: ${instruction.programId.toBase58()}`,
    );
  }
  const decoded = instructionCoder.decode(Buffer.from(instruction.data));
  if (!decoded || !isInstructionName(decoded.name)) {
    throw new Error("Unknown escrow instruction discriminator");
  }
  const expected = ACCOUNT_CONSTRAINTS[decoded.name];
  if (instruction.keys.length !== expected.length) {
    throw new Error(
      `${decoded.name} requires exactly ${expected.length} accounts; received ${instruction.keys.length}`,
    );
  }
  expected.forEach(([role, isSigner, isWritable], index) => {
    const actual = instruction.keys[index];
    if (!actual) {
      throw new Error(`${decoded.name} is missing account ${index} (${role})`);
    }
    if (actual.isSigner !== isSigner || actual.isWritable !== isWritable) {
      throw new Error(
        `${decoded.name} account ${index} (${role}) has invalid signer/writable constraints`,
      );
    }
  });
  assertCanonicalAddresses(instruction, decoded.name);
  return decoded.name;
}

export function describeEscrowInstruction(
  instruction: TransactionInstruction,
): EscrowInstructionDescription {
  const name = assertEscrowInstructionConstraints(instruction);
  const constraints = ACCOUNT_CONSTRAINTS[name];
  const dataBase58 = bs58.encode(instruction.data);
  return {
    programId: instruction.programId.toBase58(),
    name,
    accounts: instruction.keys.map((account, index) => ({
      index,
      role: constraints[index]?.[0] ?? "unknown",
      address: account.pubkey.toBase58(),
      isSigner: account.isSigner,
      isWritable: account.isWritable,
    })),
    dataBase58,
    dataBase58Sha256: sha256(dataBase58),
    dataBytesSha256: sha256(instruction.data),
  };
}

/**
 * Hashes the canonical legacy transaction message bytes. Signatures are not
 * part of `serializeMessage()`, so wallet signature differences do not change
 * this binding while any account, blockhash, fee payer, or instruction change does.
 */
export function bindLegacyTransactionMessage(
  transaction: Transaction,
): LegacyTransactionMessageBinding {
  const message = transaction.serializeMessage();
  return {
    encoding: "base64",
    messageBase64: Buffer.from(message).toString("base64"),
    messageSha256: sha256(message),
  };
}

export function assertLegacyTransactionMessageHash(
  transaction: Transaction,
  expectedSha256: string,
): void {
  if (!/^[a-f0-9]{64}$/u.test(expectedSha256)) {
    throw new Error("expectedSha256 must be a lowercase SHA-256 hex digest");
  }
  const actual = bindLegacyTransactionMessage(transaction).messageSha256;
  if (actual !== expectedSha256) {
    throw new Error(
      `Legacy transaction message hash mismatch: expected ${expectedSha256}, got ${actual}`,
    );
  }
}
