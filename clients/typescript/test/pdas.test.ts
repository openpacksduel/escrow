import { describe, expect, test } from "bun:test";
import { PublicKey } from "@solana/web3.js";
import {
  deriveCardVaultPda,
  deriveDuelPda,
  derivePaymentVaultPda,
  deriveResultCommitmentPda,
  ESCROW_PROGRAM_ID,
} from "../src/index.js";
import { fixture } from "./fixtures.js";

const encoder = new TextEncoder();

describe("PDA fixtures", () => {
  test("derives duel, payment, card, and replay receipt addresses from protocol seeds", () => {
    const nonce = new Uint8Array(8);
    new DataView(nonce.buffer).setBigUint64(0, fixture.nonce, true);
    const expectedDuel = PublicKey.findProgramAddressSync(
      [encoder.encode("duel"), fixture.creator.toBytes(), nonce],
      ESCROW_PROGRAM_ID,
    )[0];
    const duel = deriveDuelPda(fixture.creator, fixture.nonce)[0];
    expect(duel.equals(expectedDuel)).toBe(true);
    expect(
      derivePaymentVaultPda(duel)[0].equals(
        PublicKey.findProgramAddressSync(
          [encoder.encode("vault"), duel.toBytes()],
          ESCROW_PROGRAM_ID,
        )[0],
      ),
    ).toBe(true);
    expect(
      deriveCardVaultPda(duel, "creator")[0].equals(
        deriveCardVaultPda(duel, "opponent")[0],
      ),
    ).toBe(false);
    expect(
      deriveResultCommitmentPda(
        fixture.providerSigner,
        fixture.providerRequestId,
      )[0].equals(
        PublicKey.findProgramAddressSync(
          [
            encoder.encode("result"),
            fixture.providerSigner.toBytes(),
            fixture.providerRequestId,
          ],
          ESCROW_PROGRAM_ID,
        )[0],
      ),
    ).toBe(true);
  });
});
