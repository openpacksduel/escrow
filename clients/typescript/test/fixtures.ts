import { PublicKey } from "@solana/web3.js";

function fixturePublicKey(byte: number): PublicKey {
  const candidate = Uint8Array.from({ length: 32 }, () => byte);
  for (let suffix = 0; suffix <= 255; suffix += 1) {
    candidate[31] = suffix;
    if (PublicKey.isOnCurve(candidate)) {
      return new PublicKey(candidate);
    }
  }
  throw new Error(`Could not produce an on-curve fixture for byte ${byte}`);
}

export const fixture = {
  creator: fixturePublicKey(1),
  opponent: fixturePublicKey(2),
  providerSigner: fixturePublicKey(3),
  feeRecipient: fixturePublicKey(4),
  creatorCardMint: fixturePublicKey(5),
  opponentCardMint: fixturePublicKey(6),
  caller: fixturePublicKey(7),
  nonce: 42n,
  feeAmount: 1_000_000n,
  expiresAt: 2_000_000_000n,
  openedAt: 1_999_999_000n,
  valuationPolicyHash: Uint8Array.from({ length: 32 }, () => 8),
  providerRequestId: Uint8Array.from({ length: 32 }, () => 9),
};
