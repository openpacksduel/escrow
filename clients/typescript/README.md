# `@openpacksduel/escrow-client`

Devnet-only TypeScript builders for OpenPacks Duel escrow v2. The package is the
checked client boundary for the app, API, MCP server, and agents. It derives
protocol PDAs and encodes instructions with the exact IDL emitted by the
successful `main` release workflow at source SHA
`4aa3bb7560443c0565ded2d6edee67c6a544dd5f`.

This client never creates keypairs, signs transactions, submits transactions,
or holds assets. It fixes the program address and payment mint to the current
unaudited devnet deployment contract. Provider signer and fee recipient are
always supplied by the caller.

## Devnet funding flow

```ts
import {
  buildCreatorWsolFundingPlan,
  ESCROW_NETWORK,
} from "@openpacksduel/escrow-client";

const plan = buildCreatorWsolFundingPlan({
  network: ESCROW_NETWORK,
  creator,
  opponent: null,
  nonce: 42n,
  feeAmount: 1_000_000n,
  expiresAt,
  providerSigner,
  feeRecipient,
  valuationPolicyHash,
});

// The API serializes plan.instructions as one unsigned transaction.
// The creator wallet signs and sends it in the browser.
```

The creator plan idempotently creates the wallet's WSOL associated token
account, wraps the exact fee, syncs it, initializes the duel and funds the
creator side in one transaction. `buildOpponentWsolFundingPlan` wraps the same
fee and funds the opponent side. A completed match requires finalized funding
from two distinct wallets; constructing a plan is not completion.

The returned `escrowInstructions` include ordered account addresses and roles,
the canonical RPC base58 instruction data, SHA-256 of that exact base58 string,
and SHA-256 of the raw bytes. Persist these fields with the unsigned transaction,
recent blockhash expiry, and expected state intent so the transaction monitor can
verify what a wallet was asked to sign.

After assigning the fee payer and recent blockhash, call
`bindLegacyTransactionMessage(transaction)` and persist its `messageSha256`.
This hashes the complete canonical legacy message with signatures excluded.
`assertLegacyTransactionMessageHash` rejects changes to the fee payer,
blockhash, accounts, instruction ordering or data, including an inserted
instruction.

## Asset support

Card builders accept only `legacy-spl-nft`. Programmable NFTs, compressed NFTs,
and Token-2022 fail before an instruction is created. Extending this list
requires a separate on-chain custody implementation and audit.

## Artifact verification

The exact release IDL, build manifest, hashes, workflow run, and source SHA are
checked into [`idl/`](idl/). CI verifies the IDL and manifest hashes before
running the fixture suite.
