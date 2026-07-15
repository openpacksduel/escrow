import { PublicKey } from "@solana/web3.js";
import { ESCROW_PROGRAM_ID, type PlayerRole } from "./constants.js";

const encoder = new TextEncoder();

export function encodeU64LittleEndian(value: bigint): Uint8Array {
  assertUnsigned64(value, "nonce");
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, value, true);
  return bytes;
}

export function deriveDuelPda(
  creator: PublicKey,
  nonce: bigint,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [encoder.encode("duel"), creator.toBytes(), encodeU64LittleEndian(nonce)],
    ESCROW_PROGRAM_ID,
  );
}

export function derivePaymentVaultPda(duel: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [encoder.encode("vault"), duel.toBytes()],
    ESCROW_PROGRAM_ID,
  );
}

export function deriveCardVaultPda(
  duel: PublicKey,
  role: PlayerRole,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [encoder.encode("card-vault"), duel.toBytes(), encoder.encode(role)],
    ESCROW_PROGRAM_ID,
  );
}

export function deriveResultCommitmentPda(
  providerSigner: PublicKey,
  providerRequestId: Uint8Array,
): [PublicKey, number] {
  assertBytes32(providerRequestId, "providerRequestId", true);
  return PublicKey.findProgramAddressSync(
    [encoder.encode("result"), providerSigner.toBytes(), providerRequestId],
    ESCROW_PROGRAM_ID,
  );
}

export function assertUnsigned64(value: bigint, label: string): void {
  if (value < 0n || value > 18_446_744_073_709_551_615n) {
    throw new RangeError(`${label} must fit in an unsigned 64-bit integer`);
  }
}

export function assertSigned64(value: bigint, label: string): void {
  if (
    value < -9_223_372_036_854_775_808n ||
    value > 9_223_372_036_854_775_807n
  ) {
    throw new RangeError(`${label} must fit in a signed 64-bit integer`);
  }
}

export function assertBytes32(
  value: Uint8Array,
  label: string,
  nonzero = false,
): void {
  if (value.length !== 32) {
    throw new RangeError(`${label} must contain exactly 32 bytes`);
  }
  if (nonzero && value.every((byte) => byte === 0)) {
    throw new RangeError(`${label} must not be all zeroes`);
  }
}
