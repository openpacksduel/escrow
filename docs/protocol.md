# Protocol and state machine

## Goal

Two wallets commit equal, disclosed platform-fee deposits to a Solana program.
Pack purchases happen through the provider outside this program and are not
represented by those deposits. The provider later transfers one authenticated
card asset per player into protocol custody and signs an immutable result
payload. The program verifies that commitment, transfers both cards to the
winner, and routes only the fee deposits to the configured recipient.

The chain cannot call Jup, PocketPull, Collector Crypt, a TCG price API, or any
HTTP endpoint. Every off-chain fact must arrive as a signed, replay-protected
attestation whose verification rules are explicit on-chain.

## Devnet MVP state machine

```text
Waiting -> Funded -> AwaitingResult -> ReadyToSettle -> Settled
   |          |             |
   +----------+-------------+-> Refunding -> Refunded (after expiry)
   |
   +-> Cancelled (only before an opponent joins)
```

The program never chooses cards, opens packs, fetches prices, or mutates the
winner. The provider must put two supported card assets into custody and sign the
result before expiry. Once that commitment exists, settlement is deterministic
and permissionless even if the provider or application disappears.

## Current instructions

### `initialize_duel`

Creates a duel PDA and payment-vault PDA. It commits:

- creator and optional direct opponent;
- fee-payment mint and exact per-player platform-fee deposit;
- provider signer and valuation-policy hash;
- fee recipient and exact per-player fee amount;
- absolute expiry and creator-selected nonce.

The nonce allows a wallet to create multiple duels without mutable global state.
The expiry must be between one minute and seven days from initialization.

### `fund_duel`

Transfers exactly one disclosed fee deposit into the vault. For an open match, the first
non-creator depositor becomes the immutable opponent. The duel becomes `Funded`
only after both deposits succeed.

### `deposit_card_asset`

After both payments are funded, a participant or the committed provider signer
may deposit one card for a player role into that role's isolated PDA vault. The
instruction accepts only `LegacySplNft` and verifies a zero-decimal mint with a
supply of exactly one. The serialized asset-kind enum explicitly rejects pNFT,
cNFT, and Token-2022 routes.

This is a custody primitive, not proof that Collector Crypt supports a PDA as
`altPlayerAddress`. The provider integration must still prove delivery and
post-settlement marketplace, buyback, and shipping behavior.

### `submit_result`

The provider signer submits one result directly as a Solana transaction. The
commitment binds:

```text
duel PDA
provider request ID
creator and opponent wallets
creator and opponent card mints
creator and opponent integer values
valuation-policy hash
provider opening timestamp
```

The provider/request ID derives a globally unique result PDA, so reuse by the
same provider fails at account creation. The instruction also requires both
exact mints to already be in the duel vaults. Provider authorization proves the
source of the assertion, not the fairness of its inventory, randomness, or
valuation.

### `settle_duel`

Anyone may execute the committed result. The program compares the two unsigned
integer values itself. The winner receives both card assets. The two exact
fee deposits are sent only to the precommitted fee-recipient's token account.
A tie returns each original card and fee deposit and charges no fee. No pack
purchase funds or card-value-based payout are held here, and no operator override
or alternate winner path exists.

### `cancel_unmatched`

Allows the creator to recover its deposit only while no opponent deposit exists.
It cannot remove funds after an opponent has joined.

### `refund_expired_payment` and `refund_expired_card`

Before a result is committed, after expiry any signer can return each payment or
card deposit to a token account owned by its bound participant. Refund execution
is permissionless; refund ownership is not. The duel reaches `Refunded` only
after every tracked deposit leaves custody. A committed result disables refunds
because its permissionless settlement path is already final.

The refund guard checks both state and the absence of any result-commitment key.
This defense-in-depth rule means a future migration cannot accidentally reopen a
refund route merely by regressing a duel status after a valid provider result.

### `close_payment_vault` and `close_card_vault`

After the corresponding tracked deposits have left custody, any signer can
close the now-empty token vault. Closure is not privileged, but rent ownership
is fixed: payment-vault rent returns to the creator that initialized the duel,
and card-vault rent returns to the exact depositor that funded that vault's
creation. The close instruction rejects active states, tracked deposits,
non-empty vaults, substituted vaults, substituted mints, and substituted rent
recipients.

The duel and result accounts are intentionally persistent receipts. Closing and
recreating either PDA would weaken nonce/request replay guarantees and erase the
on-chain audit trail, so rent recovery applies only to custody accounts.

## Provider authorization boundary

The devnet MVP uses a direct Solana transaction signature from the provider
wallet rather than accepting a relayed detached signature. The signed
instruction data and accounts form the canonical payload, while the result PDA
preserves the immutable receipt. A relayed Ed25519-attestation instruction can
be added later without changing the result or settlement invariants.

## Fee semantics

`fee_amount` is the exact per-player platform-fee deposit committed at
initialization. On a successful non-tie settlement, both deposits go to the
precommitted fee recipient. They return to the original players on ties, refunds,
and cancellation. Card values and external pack prices never determine the fee.

## Required next gates

1. Prove Collector Crypt legacy-SPL delivery to each PDA vault on devnet using
   the checked TypeScript client and publish provider result fixtures.
2. Decide and implement pNFT/cNFT/Token-2022 custody separately, if required.
3. Add upgrade governance, reproducible builds, an external audit, and a bug
   bounty before mainnet value is accepted.

## Checked client boundary

The devnet TypeScript client in `clients/typescript` consumes the exact IDL from
the successful program release workflow at source SHA
`4aa3bb7560443c0565ded2d6edee67c6a544dd5f`. It is the canonical integration
surface for PDA derivation and instruction encoding. It also exposes an exact
ordered-account verifier and monitor representation, including base58
instruction data and stable hashes.

The client does not expand the on-chain trust boundary: it does not sign, submit,
or custody anything. It rejects every card asset standard except legacy SPL NFT
and supports only devnet WSOL fee funding. Provider signer and platform fee
recipient remain explicit per-duel inputs.
