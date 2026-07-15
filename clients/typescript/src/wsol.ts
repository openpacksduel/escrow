import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createSyncNativeInstruction,
  getAssociatedTokenAddressSync,
  NATIVE_MINT,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  type PublicKey,
  SystemProgram,
  type TransactionInstruction,
} from "@solana/web3.js";
import {
  assertDevnet,
  ESCROW_NETWORK,
  type EscrowNetwork,
} from "./constants.js";
import {
  createFundDuelInstruction,
  createInitializeDuelInstruction,
  type InitializeDuelInput,
} from "./instructions.js";
import {
  describeEscrowInstruction,
  type EscrowInstructionDescription,
} from "./monitor.js";
import { assertUnsigned64, derivePaymentVaultPda } from "./pdas.js";

export interface DevnetFundingPlan {
  network: typeof ESCROW_NETWORK;
  instructions: TransactionInstruction[];
  escrowInstructions: EscrowInstructionDescription[];
  duel: PublicKey;
  paymentVault: PublicKey;
  wrappedSolAccount: PublicKey;
  expectedStateIntent: {
    feeAmountLamports: string;
    paymentMint: string;
    player: string;
    transition: "initialize-and-fund-creator" | "fund-opponent";
  };
}

function buildWrapSolInstructions(
  owner: PublicKey,
  feeAmount: bigint,
): {
  instructions: TransactionInstruction[];
  wrappedSolAccount: PublicKey;
} {
  assertUnsigned64(feeAmount, "feeAmount");
  if (feeAmount === 0n) {
    throw new RangeError("feeAmount must be greater than zero");
  }
  if (feeAmount > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError(
      "feeAmount exceeds the safe lamport range for SystemProgram.transfer",
    );
  }
  const wrappedSolAccount = getAssociatedTokenAddressSync(
    NATIVE_MINT,
    owner,
    false,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  return {
    wrappedSolAccount,
    instructions: [
      createAssociatedTokenAccountIdempotentInstruction(
        owner,
        wrappedSolAccount,
        owner,
        NATIVE_MINT,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID,
      ),
      SystemProgram.transfer({
        fromPubkey: owner,
        toPubkey: wrappedSolAccount,
        lamports: Number(feeAmount),
      }),
      createSyncNativeInstruction(wrappedSolAccount, TOKEN_PROGRAM_ID),
    ],
  };
}

export interface CreatorWsolFundingInput extends InitializeDuelInput {
  network: EscrowNetwork;
}

export function buildCreatorWsolFundingPlan(
  input: CreatorWsolFundingInput,
): DevnetFundingPlan {
  assertDevnet(input.network);
  const wrapped = buildWrapSolInstructions(input.creator, input.feeAmount);
  const initialized = createInitializeDuelInstruction(input);
  const fund = createFundDuelInstruction({
    player: input.creator,
    playerSource: wrapped.wrappedSolAccount,
    duel: initialized.duel,
    paymentVault: initialized.paymentVault,
  });
  return {
    network: ESCROW_NETWORK,
    instructions: [...wrapped.instructions, initialized.instruction, fund],
    escrowInstructions: [
      describeEscrowInstruction(initialized.instruction),
      describeEscrowInstruction(fund),
    ],
    duel: initialized.duel,
    paymentVault: initialized.paymentVault,
    wrappedSolAccount: wrapped.wrappedSolAccount,
    expectedStateIntent: {
      feeAmountLamports: input.feeAmount.toString(),
      paymentMint: NATIVE_MINT.toBase58(),
      player: input.creator.toBase58(),
      transition: "initialize-and-fund-creator",
    },
  };
}

export interface OpponentWsolFundingInput {
  network: EscrowNetwork;
  opponent: PublicKey;
  duel: PublicKey;
  feeAmount: bigint;
}

export function buildOpponentWsolFundingPlan(
  input: OpponentWsolFundingInput,
): DevnetFundingPlan {
  assertDevnet(input.network);
  const wrapped = buildWrapSolInstructions(input.opponent, input.feeAmount);
  const paymentVault = derivePaymentVaultPda(input.duel)[0];
  const fund = createFundDuelInstruction({
    player: input.opponent,
    playerSource: wrapped.wrappedSolAccount,
    duel: input.duel,
    paymentVault,
  });
  return {
    network: ESCROW_NETWORK,
    instructions: [...wrapped.instructions, fund],
    escrowInstructions: [describeEscrowInstruction(fund)],
    duel: input.duel,
    paymentVault,
    wrappedSolAccount: wrapped.wrappedSolAccount,
    expectedStateIntent: {
      feeAmountLamports: input.feeAmount.toString(),
      paymentMint: NATIVE_MINT.toBase58(),
      player: input.opponent.toBase58(),
      transition: "fund-opponent",
    },
  };
}
