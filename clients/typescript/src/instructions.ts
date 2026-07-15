import { BN } from "@anchor-lang/core";
import { NATIVE_MINT, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  type AccountMeta,
  type PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  assertLegacySplNft,
  type CardAssetStandard,
  ESCROW_PROGRAM_ID,
  type PlayerRole,
} from "./constants.js";
import { instructionCoder } from "./idl.js";
import {
  assertBytes32,
  assertSigned64,
  assertUnsigned64,
  deriveCardVaultPda,
  deriveDuelPda,
  derivePaymentVaultPda,
  deriveResultCommitmentPda,
} from "./pdas.js";

type NamedAccount = AccountMeta & { role: string };

function writableSigner(role: string, pubkey: PublicKey): NamedAccount {
  return { role, pubkey, isSigner: true, isWritable: true };
}

function readonlySigner(role: string, pubkey: PublicKey): NamedAccount {
  return { role, pubkey, isSigner: true, isWritable: false };
}

function writable(role: string, pubkey: PublicKey): NamedAccount {
  return { role, pubkey, isSigner: false, isWritable: true };
}

function readonly(role: string, pubkey: PublicKey): NamedAccount {
  return { role, pubkey, isSigner: false, isWritable: false };
}

function buildInstruction(
  name: string,
  accounts: NamedAccount[],
  args: object,
): TransactionInstruction {
  return new TransactionInstruction({
    programId: ESCROW_PROGRAM_ID,
    keys: accounts.map(({ role: _role, ...account }) => account),
    data: instructionCoder.encode(name, args),
  });
}

function anchorRole(role: PlayerRole): object {
  return role === "creator" ? { Creator: {} } : { Opponent: {} };
}

const LEGACY_SPL_NFT = { LegacySplNft: {} };

export interface InitializeDuelInput {
  creator: PublicKey;
  nonce: bigint;
  opponent: PublicKey | null;
  feeAmount: bigint;
  expiresAt: bigint;
  providerSigner: PublicKey;
  feeRecipient: PublicKey;
  valuationPolicyHash: Uint8Array;
}

export interface DerivedDuelInstruction {
  instruction: TransactionInstruction;
  duel: PublicKey;
  paymentVault: PublicKey;
}

export function createInitializeDuelInstruction(
  input: InitializeDuelInput,
): DerivedDuelInstruction {
  assertUnsigned64(input.nonce, "nonce");
  assertUnsigned64(input.feeAmount, "feeAmount");
  if (input.feeAmount === 0n) {
    throw new RangeError("feeAmount must be greater than zero");
  }
  assertSigned64(input.expiresAt, "expiresAt");
  assertBytes32(input.valuationPolicyHash, "valuationPolicyHash", true);

  const [duel] = deriveDuelPda(input.creator, input.nonce);
  const [paymentVault] = derivePaymentVaultPda(duel);
  const instruction = buildInstruction(
    "initialize_duel",
    [
      writableSigner("creator", input.creator),
      writable("duel", duel),
      writable("payment_vault", paymentVault),
      readonly("payment_mint", NATIVE_MINT),
      readonly("token_program", TOKEN_PROGRAM_ID),
      readonly("system_program", SystemProgram.programId),
    ],
    {
      args: {
        nonce: new BN(input.nonce.toString()),
        opponent: input.opponent,
        fee_amount: new BN(input.feeAmount.toString()),
        expires_at: new BN(input.expiresAt.toString()),
        provider_signer: input.providerSigner,
        fee_recipient: input.feeRecipient,
        valuation_policy_hash: [...input.valuationPolicyHash],
      },
    },
  );

  return { instruction, duel, paymentVault };
}

export interface FundDuelInput {
  player: PublicKey;
  playerSource: PublicKey;
  duel: PublicKey;
  paymentVault?: PublicKey;
}

export function createFundDuelInstruction(
  input: FundDuelInput,
): TransactionInstruction {
  const paymentVault =
    input.paymentVault ?? derivePaymentVaultPda(input.duel)[0];
  return buildInstruction(
    "fund_duel",
    [
      writableSigner("player", input.player),
      writable("duel", input.duel),
      writable("player_source", input.playerSource),
      writable("payment_vault", paymentVault),
      readonly("payment_mint", NATIVE_MINT),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    {},
  );
}

export interface DepositCardAssetInput {
  depositor: PublicKey;
  depositorSource: PublicKey;
  duel: PublicKey;
  role: PlayerRole;
  cardMint: PublicKey;
  assetStandard: CardAssetStandard;
}

export function createDepositCardAssetInstruction(
  input: DepositCardAssetInput,
): TransactionInstruction {
  assertLegacySplNft(input.assetStandard);
  const [cardVault] = deriveCardVaultPda(input.duel, input.role);
  return buildInstruction(
    "deposit_card_asset",
    [
      writableSigner("depositor", input.depositor),
      writable("duel", input.duel),
      writable("depositor_source", input.depositorSource),
      writable("card_vault", cardVault),
      readonly("card_mint", input.cardMint),
      readonly("token_program", TOKEN_PROGRAM_ID),
      readonly("system_program", SystemProgram.programId),
    ],
    { args: { role: anchorRole(input.role), asset_kind: LEGACY_SPL_NFT } },
  );
}

export interface SubmitResultInput {
  providerSigner: PublicKey;
  duel: PublicKey;
  providerRequestId: Uint8Array;
  creator: PublicKey;
  opponent: PublicKey;
  creatorCardMint: PublicKey;
  opponentCardMint: PublicKey;
  creatorAssetStandard: CardAssetStandard;
  opponentAssetStandard: CardAssetStandard;
  valuationPolicyHash: Uint8Array;
  creatorValue: bigint;
  opponentValue: bigint;
  openedAt: bigint;
}

export interface DerivedResultInstruction {
  instruction: TransactionInstruction;
  resultCommitment: PublicKey;
}

export function createSubmitResultInstruction(
  input: SubmitResultInput,
): DerivedResultInstruction {
  assertLegacySplNft(input.creatorAssetStandard);
  assertLegacySplNft(input.opponentAssetStandard);
  assertBytes32(input.providerRequestId, "providerRequestId", true);
  assertBytes32(input.valuationPolicyHash, "valuationPolicyHash", true);
  assertUnsigned64(input.creatorValue, "creatorValue");
  assertUnsigned64(input.opponentValue, "opponentValue");
  assertSigned64(input.openedAt, "openedAt");

  const [resultCommitment] = deriveResultCommitmentPda(
    input.providerSigner,
    input.providerRequestId,
  );
  const instruction = buildInstruction(
    "submit_result",
    [
      writableSigner("provider_signer", input.providerSigner),
      writable("duel", input.duel),
      writable("result_commitment", resultCommitment),
      readonly("system_program", SystemProgram.programId),
    ],
    {
      args: {
        duel: input.duel,
        provider_request_id: [...input.providerRequestId],
        creator: input.creator,
        opponent: input.opponent,
        creator_card_mint: input.creatorCardMint,
        opponent_card_mint: input.opponentCardMint,
        creator_asset_kind: LEGACY_SPL_NFT,
        opponent_asset_kind: LEGACY_SPL_NFT,
        valuation_policy_hash: [...input.valuationPolicyHash],
        creator_value: new BN(input.creatorValue.toString()),
        opponent_value: new BN(input.opponentValue.toString()),
        opened_at: new BN(input.openedAt.toString()),
      },
    },
  );
  return { instruction, resultCommitment };
}

export interface SettleDuelInput {
  caller: PublicKey;
  duel: PublicKey;
  resultCommitment: PublicKey;
  creatorPaymentDestination: PublicKey;
  opponentPaymentDestination: PublicKey;
  feeDestination: PublicKey;
  creatorCardMint: PublicKey;
  creatorCardDestination: PublicKey;
  opponentCardMint: PublicKey;
  opponentCardDestination: PublicKey;
}

export function createSettleDuelInstruction(
  input: SettleDuelInput,
): TransactionInstruction {
  const [paymentVault] = derivePaymentVaultPda(input.duel);
  const [creatorCardVault] = deriveCardVaultPda(input.duel, "creator");
  const [opponentCardVault] = deriveCardVaultPda(input.duel, "opponent");
  return buildInstruction(
    "settle_duel",
    [
      readonlySigner("caller", input.caller),
      writable("duel", input.duel),
      writable("result_commitment", input.resultCommitment),
      writable("payment_vault", paymentVault),
      readonly("payment_mint", NATIVE_MINT),
      writable("creator_payment_destination", input.creatorPaymentDestination),
      writable(
        "opponent_payment_destination",
        input.opponentPaymentDestination,
      ),
      writable("fee_destination", input.feeDestination),
      writable("creator_card_vault", creatorCardVault),
      readonly("creator_card_mint", input.creatorCardMint),
      writable("creator_card_destination", input.creatorCardDestination),
      writable("opponent_card_vault", opponentCardVault),
      readonly("opponent_card_mint", input.opponentCardMint),
      writable("opponent_card_destination", input.opponentCardDestination),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    {},
  );
}

export interface CancelUnmatchedInput {
  creator: PublicKey;
  duel: PublicKey;
  creatorDestination: PublicKey;
}

export function createCancelUnmatchedInstruction(
  input: CancelUnmatchedInput,
): TransactionInstruction {
  return buildInstruction(
    "cancel_unmatched",
    [
      writableSigner("creator", input.creator),
      writable("duel", input.duel),
      writable("payment_vault", derivePaymentVaultPda(input.duel)[0]),
      writable("creator_destination", input.creatorDestination),
      readonly("payment_mint", NATIVE_MINT),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    {},
  );
}

export interface ClosePaymentVaultInput {
  caller: PublicKey;
  duel: PublicKey;
  rentRecipient: PublicKey;
  excessDestination: PublicKey;
}

export function createClosePaymentVaultInstruction(
  input: ClosePaymentVaultInput,
): TransactionInstruction {
  return buildInstruction(
    "close_payment_vault",
    [
      readonlySigner("caller", input.caller),
      readonly("duel", input.duel),
      writable("payment_vault", derivePaymentVaultPda(input.duel)[0]),
      readonly("payment_mint", NATIVE_MINT),
      writable("rent_recipient", input.rentRecipient),
      writable("excess_destination", input.excessDestination),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    {},
  );
}

export interface CloseCardVaultInput {
  caller: PublicKey;
  duel: PublicKey;
  role: PlayerRole;
  cardMint: PublicKey;
  rentRecipient: PublicKey;
  recoveryDestination: PublicKey;
  assetStandard: CardAssetStandard;
}

export function createCloseCardVaultInstruction(
  input: CloseCardVaultInput,
): TransactionInstruction {
  assertLegacySplNft(input.assetStandard);
  return buildInstruction(
    "close_card_vault",
    [
      readonlySigner("caller", input.caller),
      readonly("duel", input.duel),
      writable("card_vault", deriveCardVaultPda(input.duel, input.role)[0]),
      readonly("card_mint", input.cardMint),
      writable("rent_recipient", input.rentRecipient),
      writable("recovery_destination", input.recoveryDestination),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    { role: anchorRole(input.role) },
  );
}

export interface RefundExpiredPaymentInput {
  caller: PublicKey;
  duel: PublicKey;
  destination: PublicKey;
  player: PublicKey;
}

export function createRefundExpiredPaymentInstruction(
  input: RefundExpiredPaymentInput,
): TransactionInstruction {
  return buildInstruction(
    "refund_expired_payment",
    [
      readonlySigner("caller", input.caller),
      writable("duel", input.duel),
      writable("payment_vault", derivePaymentVaultPda(input.duel)[0]),
      writable("destination", input.destination),
      readonly("payment_mint", NATIVE_MINT),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    { player: input.player },
  );
}

export interface RefundExpiredCardInput {
  caller: PublicKey;
  duel: PublicKey;
  role: PlayerRole;
  cardMint: PublicKey;
  destination: PublicKey;
  assetStandard: CardAssetStandard;
}

export function createRefundExpiredCardInstruction(
  input: RefundExpiredCardInput,
): TransactionInstruction {
  assertLegacySplNft(input.assetStandard);
  return buildInstruction(
    "refund_expired_card",
    [
      readonlySigner("caller", input.caller),
      writable("duel", input.duel),
      writable("card_vault", deriveCardVaultPda(input.duel, input.role)[0]),
      readonly("card_mint", input.cardMint),
      writable("destination", input.destination),
      readonly("token_program", TOKEN_PROGRAM_ID),
    ],
    { role: anchorRole(input.role) },
  );
}
