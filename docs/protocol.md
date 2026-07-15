# Protocol and state machine

## Goal

Two wallets commit equal payment stakes to a Solana program. A pack provider
later opens one authenticated pack per player, transfers the resulting card
assets into protocol custody, and signs an immutable result payload. The program
verifies that commitment and atomically pays the winner, transfers both card
sets, and routes the configured fee.

The chain cannot call Jup, PocketPull, Collector Crypt, a TCG price API, or any
HTTP endpoint. Every off-chain fact must arrive as a signed, replay-protected
attestation whose verification rules are explicit on-chain.

## Intended end-to-end states

```text
Waiting -> Funded -> Opening -> AwaitingAssets -> Settled
   |          |           |            |
   +----------+-----------+------------+-> Refunded (after the relevant deadline)
   |
   +-> Cancelled (only before an opponent joins)
```

The first program slice implements `Waiting`, `Funded`, `Cancelled`, and
`Refunded`. The opening and settlement states remain deliberately absent until
the provider payload and NFT custody model are specified.

## Current instructions

### `initialize_duel`

Creates a duel PDA and token-vault PDA. It commits:

- creator and optional direct opponent;
- payment mint and equal per-player stake;
- provider signer and valuation-policy hash;
- fee recipient and fee basis points;
- absolute expiry and creator-selected nonce.

The nonce allows a wallet to create multiple duels without mutable global state.
The expiry must be between one minute and seven days from initialization.

### `fund_duel`

Transfers exactly one stake into the vault. For an open match, the first
non-creator depositor becomes the immutable opponent. The duel becomes `Funded`
only after both deposits succeed.

### `cancel_unmatched`

Allows the creator to recover its deposit only while no opponent deposit exists.
It cannot remove funds after an opponent has joined.

### `refund_expired`

After expiry, any signer can return one participant's deposit to a token account
owned by that participant. Refund execution is permissionless; refund ownership
is not. Calling once per funded participant fully refunds the duel.

## Settlement payload (next protocol milestone)

The provider attestation should use a versioned canonical byte layout containing
at least:

```text
domain_separator
program_id
duel_pda
provider_request_id
pack_definition_id
creator_asset_ids[]
opponent_asset_ids[]
creator_value_minor_units
opponent_value_minor_units
valuation_policy_hash
opened_at
expires_at
```

The settlement transaction must inspect an Ed25519 verification instruction,
reject reused provider request IDs, enforce the committed policy hash, verify
every asset against custody accounts, define tie behavior, and transfer value
atomically. A backend signature by itself must never bypass those checks.

## Fee semantics

`fee_bps` is committed at initialization and capped at 10%. The current program
does not charge it. Before settlement ships, the specification must decide:

- whether the fee applies to payment stakes, card value, or pack purchase price;
- whether ties are fee-free;
- rounding direction and dust ownership;
- whether fees are collected only on successful settlement.

The recommended MVP is a fee on successfully settled payment stakes only, with
rounding down and no fee on refund or cancellation.

## Required next gates

1. Publish the provider attestation schema and test vectors.
2. Decide whether cards are Metaplex Core assets, Token-2022 NFTs, or a
   provider-controlled redemption receipt.
3. Implement verified asset custody and atomic winner settlement.
4. Add upgrade governance, reproducible builds, an external audit, and a bug
   bounty before mainnet value is accepted.
