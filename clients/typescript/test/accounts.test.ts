import { describe, expect, test } from "bun:test";
import { Buffer } from "node:buffer";
import { BN, BorshAccountsCoder } from "@anchor-lang/core";
import { NATIVE_MINT } from "@solana/spl-token";
import { ESCROW_IDL } from "../src/idl.js";
import {
  decodeDuelAccount,
  ESCROW_DUEL_ACCOUNT_SIZE,
  ESCROW_DUEL_VERSION,
} from "../src/index.js";
import { deriveCardVaultPda, derivePaymentVaultPda } from "../src/pdas.js";
import { fixture } from "./fixtures.js";

const accountCoder = new BorshAccountsCoder(ESCROW_IDL);

async function encodedDuel(): Promise<Buffer> {
  return accountCoder.encode("Duel", {
    version: ESCROW_DUEL_VERSION,
    bump: 254,
    payment_vault_bump: 253,
    status: { ReadyToSettle: {} },
    creator: fixture.creator,
    opponent: fixture.opponent,
    payment_mint: NATIVE_MINT,
    payment_vault: derivePaymentVaultPda(fixture.caller)[0],
    fee_recipient: fixture.feeRecipient,
    provider_signer: fixture.providerSigner,
    nonce: new BN(fixture.nonce.toString()),
    fee_amount: new BN(fixture.feeAmount.toString()),
    created_at: new BN((fixture.expiresAt - 1_000n).toString()),
    expires_at: new BN(fixture.expiresAt.toString()),
    creator_deposited: true,
    opponent_deposited: true,
    creator_card_deposited: true,
    opponent_card_deposited: true,
    creator_card_mint: fixture.creatorCardMint,
    opponent_card_mint: fixture.opponentCardMint,
    creator_card_vault: deriveCardVaultPda(fixture.caller, "creator")[0],
    opponent_card_vault: deriveCardVaultPda(fixture.caller, "opponent")[0],
    creator_card_rent_recipient: fixture.creator,
    opponent_card_rent_recipient: fixture.opponent,
    creator_card_terminal_beneficiary: fixture.creator,
    opponent_card_terminal_beneficiary: fixture.creator,
    result_commitment: fixture.caller,
    valuation_policy_hash: [...fixture.valuationPolicyHash],
  });
}

describe("Duel v4 account decoder", () => {
  test("decodes the exact 560-byte generated-IDL layout", async () => {
    const encoded = await encodedDuel();
    expect(encoded).toHaveLength(ESCROW_DUEL_ACCOUNT_SIZE);

    const duel = decodeDuelAccount(encoded);
    expect(duel.version).toBe(4);
    expect(duel.status).toBe("ready-to-settle");
    expect(duel.nonce).toBe(fixture.nonce);
    expect(duel.feeAmount).toBe(fixture.feeAmount);
    expect(duel.creatorCardRentRecipient.equals(fixture.creator)).toBe(true);
    expect(duel.opponentCardRentRecipient.equals(fixture.opponent)).toBe(true);
    expect(duel.creatorCardTerminalBeneficiary.equals(fixture.creator)).toBe(
      true,
    );
    expect(duel.opponentCardTerminalBeneficiary.equals(fixture.creator)).toBe(
      true,
    );
    expect(duel.valuationPolicyHash).toEqual(fixture.valuationPolicyHash);
  });

  test("rejects legacy, oversized, wrong-version, and wrong-discriminator data", async () => {
    const encoded = await encodedDuel();
    expect(() => decodeDuelAccount(encoded.subarray(0, 432))).toThrow(
      "exactly 560 bytes",
    );
    expect(() =>
      decodeDuelAccount(Buffer.concat([encoded, Buffer.of(0)])),
    ).toThrow("exactly 560 bytes");

    const wrongVersion = Buffer.from(encoded);
    wrongVersion[8] = 3;
    expect(() => decodeDuelAccount(wrongVersion)).toThrow(
      "Unsupported Duel account version: 3",
    );

    const wrongDiscriminator = Buffer.from(encoded);
    wrongDiscriminator[0] = (wrongDiscriminator[0] ?? 0) ^ 0xff;
    expect(() => decodeDuelAccount(wrongDiscriminator)).toThrow(
      "invalid discriminator",
    );
  });
});
