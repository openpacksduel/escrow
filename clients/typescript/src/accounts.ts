import { Buffer } from "node:buffer";
import type { BN } from "@anchor-lang/core";
import type { PublicKey } from "@solana/web3.js";
import {
  ESCROW_DUEL_ACCOUNT_SIZE,
  ESCROW_DUEL_DISCRIMINATOR,
  ESCROW_DUEL_VERSION,
} from "./constants.js";
import { accountCoder } from "./idl.js";

const STATUS_BY_VARIANT = {
  Waiting: "waiting",
  Funded: "funded",
  AwaitingResult: "awaiting-result",
  ReadyToSettle: "ready-to-settle",
  Refunding: "refunding",
  Settled: "settled",
  Cancelled: "cancelled",
  Refunded: "refunded",
} as const;

export type DuelStatus =
  (typeof STATUS_BY_VARIANT)[keyof typeof STATUS_BY_VARIANT];

export interface DecodedDuelAccount {
  version: typeof ESCROW_DUEL_VERSION;
  bump: number;
  paymentVaultBump: number;
  status: DuelStatus;
  creator: PublicKey;
  opponent: PublicKey;
  paymentMint: PublicKey;
  paymentVault: PublicKey;
  feeRecipient: PublicKey;
  providerSigner: PublicKey;
  nonce: bigint;
  feeAmount: bigint;
  createdAt: bigint;
  expiresAt: bigint;
  creatorDeposited: boolean;
  opponentDeposited: boolean;
  creatorCardDeposited: boolean;
  opponentCardDeposited: boolean;
  creatorCardMint: PublicKey;
  opponentCardMint: PublicKey;
  creatorCardVault: PublicKey;
  opponentCardVault: PublicKey;
  creatorCardRentRecipient: PublicKey;
  opponentCardRentRecipient: PublicKey;
  creatorCardTerminalBeneficiary: PublicKey;
  opponentCardTerminalBeneficiary: PublicKey;
  resultCommitment: PublicKey;
  valuationPolicyHash: Uint8Array;
}

interface RawDuelAccount {
  version: number;
  bump: number;
  payment_vault_bump: number;
  status: unknown;
  creator: PublicKey;
  opponent: PublicKey;
  payment_mint: PublicKey;
  payment_vault: PublicKey;
  fee_recipient: PublicKey;
  provider_signer: PublicKey;
  nonce: BN;
  fee_amount: BN;
  created_at: BN;
  expires_at: BN;
  creator_deposited: boolean;
  opponent_deposited: boolean;
  creator_card_deposited: boolean;
  opponent_card_deposited: boolean;
  creator_card_mint: PublicKey;
  opponent_card_mint: PublicKey;
  creator_card_vault: PublicKey;
  opponent_card_vault: PublicKey;
  creator_card_rent_recipient: PublicKey;
  opponent_card_rent_recipient: PublicKey;
  creator_card_terminal_beneficiary: PublicKey;
  opponent_card_terminal_beneficiary: PublicKey;
  result_commitment: PublicKey;
  valuation_policy_hash: number[];
}

function hasExactDiscriminator(data: Uint8Array): boolean {
  return ESCROW_DUEL_DISCRIMINATOR.every(
    (value, index) => data[index] === value,
  );
}

function decodeStatus(value: unknown): DuelStatus {
  if (typeof value !== "object" || value === null) {
    throw new Error("Duel account contains an invalid status");
  }
  const variants = Object.keys(value);
  if (variants.length !== 1) {
    throw new Error("Duel account contains an invalid status");
  }
  const variant = variants[0];
  if (!variant || !(variant in STATUS_BY_VARIANT)) {
    throw new Error("Duel account contains an unknown status");
  }
  return STATUS_BY_VARIANT[variant as keyof typeof STATUS_BY_VARIANT];
}

function toBigInt(value: BN): bigint {
  return BigInt(value.toString());
}

if (
  accountCoder.size("Duel") !== ESCROW_DUEL_ACCOUNT_SIZE ||
  !accountCoder
    .accountDiscriminator("Duel")
    .equals(Buffer.from(ESCROW_DUEL_DISCRIMINATOR))
) {
  throw new Error("Checked escrow IDL has an unexpected Duel account ABI");
}

export function decodeDuelAccount(data: Uint8Array): DecodedDuelAccount {
  if (data.length !== ESCROW_DUEL_ACCOUNT_SIZE) {
    throw new Error(
      `Duel account must be exactly ${ESCROW_DUEL_ACCOUNT_SIZE} bytes; received ${data.length}`,
    );
  }
  if (!hasExactDiscriminator(data)) {
    throw new Error("Duel account has an invalid discriminator");
  }

  const raw = accountCoder.decode("Duel", Buffer.from(data)) as RawDuelAccount;
  if (raw.version !== ESCROW_DUEL_VERSION) {
    throw new Error(
      `Unsupported Duel account version: ${raw.version}; expected ${ESCROW_DUEL_VERSION}`,
    );
  }

  return {
    version: ESCROW_DUEL_VERSION,
    bump: raw.bump,
    paymentVaultBump: raw.payment_vault_bump,
    status: decodeStatus(raw.status),
    creator: raw.creator,
    opponent: raw.opponent,
    paymentMint: raw.payment_mint,
    paymentVault: raw.payment_vault,
    feeRecipient: raw.fee_recipient,
    providerSigner: raw.provider_signer,
    nonce: toBigInt(raw.nonce),
    feeAmount: toBigInt(raw.fee_amount),
    createdAt: toBigInt(raw.created_at),
    expiresAt: toBigInt(raw.expires_at),
    creatorDeposited: raw.creator_deposited,
    opponentDeposited: raw.opponent_deposited,
    creatorCardDeposited: raw.creator_card_deposited,
    opponentCardDeposited: raw.opponent_card_deposited,
    creatorCardMint: raw.creator_card_mint,
    opponentCardMint: raw.opponent_card_mint,
    creatorCardVault: raw.creator_card_vault,
    opponentCardVault: raw.opponent_card_vault,
    creatorCardRentRecipient: raw.creator_card_rent_recipient,
    opponentCardRentRecipient: raw.opponent_card_rent_recipient,
    creatorCardTerminalBeneficiary: raw.creator_card_terminal_beneficiary,
    opponentCardTerminalBeneficiary: raw.opponent_card_terminal_beneficiary,
    resultCommitment: raw.result_commitment,
    valuationPolicyHash: Uint8Array.from(raw.valuation_policy_hash),
  };
}
