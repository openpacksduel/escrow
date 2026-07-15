import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { SystemInstruction, SystemProgram, Transaction } from "@solana/web3.js";
import {
  assertLegacyTransactionMessageHash,
  bindLegacyTransactionMessage,
  buildCreatorWsolFundingPlan,
  buildOpponentWsolFundingPlan,
  ESCROW_NETWORK,
} from "../src/index.js";
import { fixture } from "./fixtures.js";

describe("devnet WSOL funding fixtures", () => {
  test("builds one atomic creator initialize-and-fund plan", () => {
    const plan = buildCreatorWsolFundingPlan({
      network: ESCROW_NETWORK,
      creator: fixture.creator,
      nonce: fixture.nonce,
      opponent: fixture.opponent,
      feeAmount: fixture.feeAmount,
      expiresAt: fixture.expiresAt,
      providerSigner: fixture.providerSigner,
      feeRecipient: fixture.feeRecipient,
      valuationPolicyHash: fixture.valuationPolicyHash,
    });

    expect(plan.instructions).toHaveLength(5);
    expect(plan.escrowInstructions.map(({ name }) => name)).toEqual([
      "initialize_duel",
      "fund_duel",
    ]);
    const transfer = plan.instructions[1];
    expect(transfer).toBeDefined();
    if (!transfer) {
      throw new Error("Creator plan is missing the WSOL transfer instruction");
    }
    expect(SystemInstruction.decodeTransfer(transfer).lamports).toBe(
      fixture.feeAmount,
    );
    expect(plan.expectedStateIntent).toEqual({
      feeAmountLamports: "1000000",
      paymentMint: "So11111111111111111111111111111111111111112",
      player: fixture.creator.toBase58(),
      transition: "initialize-and-fund-creator",
    });
  });

  test("builds the opponent wrap-and-fund plan against the same vault", () => {
    const creatorPlan = buildCreatorWsolFundingPlan({
      network: ESCROW_NETWORK,
      creator: fixture.creator,
      nonce: fixture.nonce,
      opponent: null,
      feeAmount: fixture.feeAmount,
      expiresAt: fixture.expiresAt,
      providerSigner: fixture.providerSigner,
      feeRecipient: fixture.feeRecipient,
      valuationPolicyHash: fixture.valuationPolicyHash,
    });
    const opponentPlan = buildOpponentWsolFundingPlan({
      network: ESCROW_NETWORK,
      opponent: fixture.opponent,
      duel: creatorPlan.duel,
      feeAmount: fixture.feeAmount,
    });

    expect(opponentPlan.instructions).toHaveLength(4);
    expect(opponentPlan.paymentVault.equals(creatorPlan.paymentVault)).toBe(
      true,
    );
    expect(opponentPlan.escrowInstructions[0]?.name).toBe("fund_duel");
  });

  test("binds every unsigned legacy-message byte and rejects mutations or extra instructions", () => {
    const plan = buildCreatorWsolFundingPlan({
      network: ESCROW_NETWORK,
      creator: fixture.creator,
      nonce: fixture.nonce,
      opponent: fixture.opponent,
      feeAmount: fixture.feeAmount,
      expiresAt: fixture.expiresAt,
      providerSigner: fixture.providerSigner,
      feeRecipient: fixture.feeRecipient,
      valuationPolicyHash: fixture.valuationPolicyHash,
    });
    const makeTransaction = () =>
      new Transaction({
        feePayer: fixture.creator,
        recentBlockhash: fixture.caller.toBase58(),
      }).add(...plan.instructions);

    const transaction = makeTransaction();
    const binding = bindLegacyTransactionMessage(transaction);
    expect(binding.encoding).toBe("base64");
    expect(binding.messageSha256).toBe(
      createHash("sha256").update(transaction.serializeMessage()).digest("hex"),
    );
    expect(() =>
      assertLegacyTransactionMessageHash(transaction, binding.messageSha256),
    ).not.toThrow();

    const mutated = makeTransaction();
    const firstData = mutated.instructions[0]?.data;
    if (!firstData) {
      throw new Error("Fixture transaction is missing its first instruction");
    }
    firstData[0] = (firstData[0] ?? 0) ^ 1;
    expect(() =>
      assertLegacyTransactionMessageHash(mutated, binding.messageSha256),
    ).toThrow("message hash mismatch");

    const extraInstruction = makeTransaction().add(
      SystemProgram.transfer({
        fromPubkey: fixture.creator,
        toPubkey: fixture.opponent,
        lamports: 1,
      }),
    );
    expect(() =>
      assertLegacyTransactionMessageHash(
        extraInstruction,
        binding.messageSha256,
      ),
    ).toThrow("message hash mismatch");
  });
});
