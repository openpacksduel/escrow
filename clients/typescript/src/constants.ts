import { PublicKey } from "@solana/web3.js";

export const ESCROW_PROGRAM_ID = new PublicKey(
  "Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS",
);

export const ESCROW_NETWORK = "solana-devnet" as const;

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
