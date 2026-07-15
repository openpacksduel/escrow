import { describe, expect, test } from "bun:test";
import {
  assertEscrowInstructionConstraints,
  createDepositCardAssetInstruction,
  createInitializeDuelInstruction,
  createRefundExpiredCardInstruction,
  createRefundExpiredPaymentInstruction,
  createSubmitResultInstruction,
  describeEscrowInstruction,
  ESCROW_PROGRAM_ID,
} from "../src/index.js";
import { fixture } from "./fixtures.js";

function littleEndian64(value: bigint): Uint8Array {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, value, true);
  return bytes;
}

function bytes(...parts: ArrayLike<number>[]): Uint8Array {
  const result = new Uint8Array(
    parts.reduce((length, part) => length + part.length, 0),
  );
  let offset = 0;
  for (const part of parts) {
    result.set(Array.from(part), offset);
    offset += part.length;
  }
  return result;
}

describe("instruction fixtures", () => {
  test("encodes initialize_duel from the checked IDL with exact ordered constraints", () => {
    const { instruction, duel, paymentVault } = createInitializeDuelInstruction(
      {
        creator: fixture.creator,
        nonce: fixture.nonce,
        opponent: fixture.opponent,
        feeAmount: fixture.feeAmount,
        expiresAt: fixture.expiresAt,
        providerSigner: fixture.providerSigner,
        feeRecipient: fixture.feeRecipient,
        valuationPolicyHash: fixture.valuationPolicyHash,
      },
    );
    const description = describeEscrowInstruction(instruction);

    expect(instruction.programId.equals(ESCROW_PROGRAM_ID)).toBe(true);
    expect([...instruction.data.subarray(0, 8)]).toEqual([
      197, 5, 158, 89, 174, 188, 134, 6,
    ]);
    expect(description.name).toBe("initialize_duel");
    expect(description.accounts.map(({ role }) => role)).toEqual([
      "creator",
      "duel",
      "payment_vault",
      "payment_mint",
      "token_program",
      "system_program",
    ]);
    expect(description.accounts[1]?.address).toBe(duel.toBase58());
    expect(description.accounts[2]?.address).toBe(paymentVault.toBase58());
    expect(description.dataBase58Sha256).toHaveLength(64);
    expect(description.dataBytesSha256).toHaveLength(64);
    expect(instruction.data).toEqual(
      bytes(
        [197, 5, 158, 89, 174, 188, 134, 6],
        littleEndian64(fixture.nonce),
        [1],
        fixture.opponent.toBytes(),
        littleEndian64(fixture.feeAmount),
        littleEndian64(fixture.expiresAt),
        fixture.providerSigner.toBytes(),
        fixture.feeRecipient.toBytes(),
        fixture.valuationPolicyHash,
      ),
    );
  });

  test("encodes the None opponent option as the Rust Borsh zero tag", () => {
    const { instruction } = createInitializeDuelInstruction({
      creator: fixture.creator,
      nonce: fixture.nonce,
      opponent: null,
      feeAmount: fixture.feeAmount,
      expiresAt: fixture.expiresAt,
      providerSigner: fixture.providerSigner,
      feeRecipient: fixture.feeRecipient,
      valuationPolicyHash: fixture.valuationPolicyHash,
    });
    expect(instruction.data[16]).toBe(0);
    expect(instruction.data.length).toBe(129);
  });

  test("encodes both player-role vectors and legacy asset kind with Rust enum indices", () => {
    const creator = createDepositCardAssetInstruction({
      depositor: fixture.creator,
      depositorSource: fixture.creator,
      duel: fixture.caller,
      role: "creator",
      cardMint: fixture.creatorCardMint,
      assetStandard: "legacy-spl-nft",
    });
    const opponent = createDepositCardAssetInstruction({
      depositor: fixture.opponent,
      depositorSource: fixture.opponent,
      duel: fixture.caller,
      role: "opponent",
      cardMint: fixture.opponentCardMint,
      assetStandard: "legacy-spl-nft",
    });
    expect(creator.data).toEqual(
      bytes([212, 169, 85, 35, 162, 91, 119, 42], [0, 0]),
    );
    expect(opponent.data).toEqual(
      bytes([212, 169, 85, 35, 162, 91, 119, 42], [1, 0]),
    );
  });

  test("fails closed for unsupported card standards", () => {
    expect(() =>
      createDepositCardAssetInstruction({
        depositor: fixture.creator,
        depositorSource: fixture.creator,
        duel: fixture.caller,
        role: "creator",
        cardMint: fixture.creatorCardMint,
        assetStandard: "programmable-nft",
      }),
    ).toThrow("Unsupported card asset standard");
  });

  test("binds submit_result to the provider request replay PDA", () => {
    const { instruction, resultCommitment } = createSubmitResultInstruction({
      providerSigner: fixture.providerSigner,
      duel: fixture.caller,
      providerRequestId: fixture.providerRequestId,
      creator: fixture.creator,
      opponent: fixture.opponent,
      creatorCardMint: fixture.creatorCardMint,
      opponentCardMint: fixture.opponentCardMint,
      creatorAssetStandard: "legacy-spl-nft",
      opponentAssetStandard: "legacy-spl-nft",
      valuationPolicyHash: fixture.valuationPolicyHash,
      creatorValue: 2_000_000n,
      opponentValue: 1_500_000n,
      openedAt: fixture.openedAt,
    });

    expect([...instruction.data.subarray(0, 8)]).toEqual([
      240, 42, 89, 180, 10, 239, 9, 214,
    ]);
    expect(instruction.keys[2]?.pubkey.equals(resultCommitment)).toBe(true);
    expect(assertEscrowInstructionConstraints(instruction)).toBe(
      "submit_result",
    );
    expect(instruction.data).toEqual(
      bytes(
        [240, 42, 89, 180, 10, 239, 9, 214],
        fixture.caller.toBytes(),
        fixture.providerRequestId,
        fixture.creator.toBytes(),
        fixture.opponent.toBytes(),
        fixture.creatorCardMint.toBytes(),
        fixture.opponentCardMint.toBytes(),
        [0, 0],
        fixture.valuationPolicyHash,
        littleEndian64(2_000_000n),
        littleEndian64(1_500_000n),
        littleEndian64(fixture.openedAt),
      ),
    );
  });

  test("encodes both refund variants against independent Borsh fixtures", () => {
    const card = createRefundExpiredCardInstruction({
      caller: fixture.caller,
      duel: fixture.creator,
      role: "opponent",
      cardMint: fixture.opponentCardMint,
      destination: fixture.opponent,
      assetStandard: "legacy-spl-nft",
    });
    const payment = createRefundExpiredPaymentInstruction({
      caller: fixture.caller,
      duel: fixture.creator,
      destination: fixture.opponent,
      player: fixture.opponent,
    });
    expect(card.data).toEqual(
      bytes([160, 130, 63, 132, 223, 30, 235, 144], [1]),
    );
    expect(payment.data).toEqual(
      bytes([82, 5, 192, 101, 25, 133, 163, 209], fixture.opponent.toBytes()),
    );
  });

  test("rejects altered signer constraints before monitor persistence", () => {
    const { instruction } = createInitializeDuelInstruction({
      creator: fixture.creator,
      nonce: fixture.nonce,
      opponent: null,
      feeAmount: fixture.feeAmount,
      expiresAt: fixture.expiresAt,
      providerSigner: fixture.providerSigner,
      feeRecipient: fixture.feeRecipient,
      valuationPolicyHash: fixture.valuationPolicyHash,
    });
    const tampered = {
      ...instruction,
      programId: instruction.programId,
      keys: instruction.keys.map((account, index) =>
        index === 0 ? { ...account, isSigner: false } : account,
      ),
      data: Buffer.from(instruction.data),
    };

    expect(() => assertEscrowInstructionConstraints(tampered)).toThrow(
      "invalid signer/writable",
    );
  });

  test("rejects zero replay IDs", () => {
    expect(() =>
      createSubmitResultInstruction({
        providerSigner: fixture.providerSigner,
        duel: fixture.caller,
        providerRequestId: new Uint8Array(32),
        creator: fixture.creator,
        opponent: fixture.opponent,
        creatorCardMint: fixture.creatorCardMint,
        opponentCardMint: fixture.opponentCardMint,
        creatorAssetStandard: "legacy-spl-nft",
        opponentAssetStandard: "legacy-spl-nft",
        valuationPolicyHash: fixture.valuationPolicyHash,
        creatorValue: 1n,
        opponentValue: 2n,
        openedAt: fixture.openedAt,
      }),
    ).toThrow("must not be all zeroes");
  });
});
