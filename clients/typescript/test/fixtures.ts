import { PublicKey } from "@solana/web3.js";

export const fixture = {
  creator: new PublicKey(Uint8Array.from({ length: 32 }, () => 1)),
  opponent: new PublicKey(Uint8Array.from({ length: 32 }, () => 2)),
  providerSigner: new PublicKey(Uint8Array.from({ length: 32 }, () => 3)),
  feeRecipient: new PublicKey(Uint8Array.from({ length: 32 }, () => 4)),
  creatorCardMint: new PublicKey(Uint8Array.from({ length: 32 }, () => 5)),
  opponentCardMint: new PublicKey(Uint8Array.from({ length: 32 }, () => 6)),
  caller: new PublicKey(Uint8Array.from({ length: 32 }, () => 7)),
  nonce: 42n,
  feeAmount: 1_000_000n,
  expiresAt: 2_000_000_000n,
  openedAt: 1_999_999_000n,
  valuationPolicyHash: Uint8Array.from({ length: 32 }, () => 8),
  providerRequestId: Uint8Array.from({ length: 32 }, () => 9),
};
