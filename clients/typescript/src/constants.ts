import { PublicKey } from "@solana/web3.js";

export const ESCROW_PROGRAM_ID = new PublicKey(
  "Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS",
);

export const ESCROW_NETWORK = "solana-devnet" as const;
export const ESCROW_DUEL_VERSION = 4 as const;
export const ESCROW_DUEL_ACCOUNT_SIZE = 560 as const;
export const ESCROW_DUEL_DISCRIMINATOR = [
  126, 229, 210, 60, 177, 135, 124, 224,
] as const;

export type EscrowNetwork = typeof ESCROW_NETWORK;
export type PlayerRole = "creator" | "opponent";
export type CardAssetStandard =
  | "legacy-spl-nft"
  | "programmable-nft"
  | "compressed-nft"
  | "token-2022";

export function assertDevnet(network: EscrowNetwork): void {
  if (network !== ESCROW_NETWORK) {
    throw new Error("This unaudited escrow client supports Solana devnet only");
  }
}

export function assertLegacySplNft(assetStandard: CardAssetStandard): void {
  if (assetStandard !== "legacy-spl-nft") {
    throw new Error(
      `Unsupported card asset standard: ${assetStandard}. The devnet escrow accepts only legacy SPL NFTs.`,
    );
  }
}
