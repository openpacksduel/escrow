# Threat model

## Protected assets

- both players' disclosed platform-fee deposits (not pack-purchase funds);
- card assets or redemption rights produced by each pack;
- provider result integrity and uniqueness;
- the protocol fee destination;
- availability of refunds when an operator or provider fails.

## Trust boundaries

### Solana program

The program is the final authority for custody and settlement. Users should be
able to reconstruct a duel from account data and events without trusting the web
application.

### Pack provider

The provider controls pack inventory, randomness, opening results, and card
delivery. On-chain signatures prove which provider asserted a result; they do
not prove that the provider's randomness or inventory is fair. Provider policy,
commit-reveal evidence, and auditability are separate product requirements.

### Valuation service

Card prices are off-chain and manipulable. A duel commits a valuation-policy
hash before either pack opens. The future result must bind asset IDs, price
source, price timestamp, currency, condition, and rounding rules to that hash.

### Web app and matchmaking service

These services may relay transactions and index events, but must never custody
player keys or have unilateral withdrawal authority. Clients must display the
decoded transaction and committed duel terms before wallet approval.

## Primary attacks and controls

| Threat | Required control |
| --- | --- |
| Fake or replayed pack result | Provider must sign the Solana submission; the result binds the exact duel/players/mints/policy/values; provider/request ID derives a globally unique PDA |
| Matchmaking front-run | Direct challenges bind an opponent; open matches explicitly accept first-depositor semantics |
| Operator disappearance | Permissionless deadline refunds to participant-owned accounts |
| Creator cancels after opponent joins | Cancellation rejects any duel with an opponent deposit |
| Malicious refund destination | Destination token owner must equal the refunded player |
| Unsupported token behavior breaks custody | Account owners must be the legacy SPL Token Program; asset kind must be `LegacySplNft`; mint must have zero decimals and supply one. pNFT/cNFT/Token-2022 are rejected or cannot satisfy the account schema |
| Price manipulation | Precommitted policy hash, bounded quote age, multiple-source/fallback rules, integer minor units |
| NFT substitution | Provider result binds the exact mints already held by the two role-specific PDA vaults. Collection/metadata verification remains a documented devnet limitation |
| Fee-recipient or amount swap | Recipient and exact per-player fee amount are committed in duel state before funding |
| App treats fee vault as pack funding | Contract state stores an exact `fee_amount`; protocol docs exclude pack purchases and winner payment payouts from this program |
| Upgrade-authority compromise | Multisig/timelock governance, published upgrade policy, verified builds |
| Stuck state after provider timeout | Before result commitment, expiry permits independent permissionless payment/card refunds. After commitment, deterministic settlement is permissionless |
| Provider changes outcome | One immutable result account per duel plus a globally unique provider/request receipt; no update instruction or privileged winner override exists |
| Settlement caller redirects assets | Every payment/card destination owner is checked against the deterministic winner or original owner; fee destination is checked against the committed recipient |
| Duplicate mutable-account aliasing | Anchor account constraints plus explicit participant and destination checks |

## Known devnet MVP gaps

- Provider authorization is a direct Solana signer, not a relayed detached
  attestation with published cross-language test vectors.
- Only legacy SPL, zero-decimal, single-supply mints are supported. Metaplex
  collection/metadata provenance is not verified on-chain, and pNFTs, cNFTs,
  Metaplex Core, and Token-2022 are unsupported.
- One deadline covers funding, custody, and provider result submission. Once a
  result is committed, settlement intentionally has no deadline.
- Payment-mint allowlisting is not yet governed on-chain.
- The devnet program ID is reserved but deployment awaits a funded authority.
- Unsolicited legacy SPL transfers can leave untracked dust in PDA vaults.
- The program has not received an independent audit.

No mainnet deployment should accept value while these gaps remain.
