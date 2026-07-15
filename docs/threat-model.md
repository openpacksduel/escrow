# Threat model

## Protected assets

- both players' payment deposits;
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
| Fake or replayed pack result | Domain-separated provider signature, duel PDA and program ID in payload, request-ID replay account |
| Matchmaking front-run | Direct challenges bind an opponent; open matches explicitly accept first-depositor semantics |
| Operator disappearance | Permissionless deadline refunds to participant-owned accounts |
| Creator cancels after opponent joins | Cancellation rejects any duel with an opponent deposit |
| Malicious refund destination | Destination token owner must equal the refunded player |
| Token-2022 transfer fee breaks accounting | MVP accepts legacy SPL Token Program only |
| Price manipulation | Precommitted policy hash, bounded quote age, multiple-source/fallback rules, integer minor units |
| NFT substitution | Attestation binds exact asset IDs; settlement verifies mint/collection/owner and vault custody |
| Fee-recipient swap | Recipient and basis points committed in duel state before funding |
| Upgrade-authority compromise | Multisig/timelock governance, published upgrade policy, verified builds |
| Stuck state after provider timeout | Separate matchmaking, opening, and settlement deadlines with permissionless unwind paths |
| Duplicate mutable-account aliasing | Anchor account constraints plus explicit participant and destination checks |

## Known gaps in this foundation

- No provider-signature verification or replay protection yet.
- No card/NFT custody or winner settlement yet.
- One deadline currently covers the pre-settlement lifecycle.
- Payment-mint allowlisting is not yet governed on-chain.
- The development program ID has no deployment authority policy.
- The program has not received an independent audit.

No mainnet deployment should accept value while these gaps remain.
