# OpenPacks Duel Escrow

Open-source Solana escrow for two-player pack-opening duels.

This repository is the public protocol boundary for OpenPacks Duel. The web app,
matchmaking service, and pack-provider adapter live separately in
[`openpacksduel/app`](https://github.com/openpacksduel/app).

> [!WARNING]
> This is an unaudited foundation, not a production deployment. The current
> program supports duel creation, equal SPL-token deposits, unmatched
> cancellation, and permissionless expiry refunds. Provider attestations, NFT
> custody, winner settlement, governance, and verified builds remain gated work.

## Current contract

- A creator opens either a direct challenge or an open match.
- Both players deposit the same amount of a legacy SPL token into a PDA vault.
- An open match binds its opponent when the first non-creator deposits.
- A creator can cancel only before an opponent has joined.
- After the deadline, anyone can trigger a refund to either player's owned token
  account, so neither player depends on an operator.
- The configured provider signer, fee recipient, fee rate, and valuation-policy
  hash are committed to duel state for the future settlement instruction.

The initial payment implementation intentionally targets the legacy SPL Token
Program. Token-2022 transfer-fee mints are excluded until vault accounting can
verify net received amounts.

## Repository layout

```text
programs/openpacksduel-escrow/  Anchor program
docs/protocol.md                State machine and trust boundaries
docs/threat-model.md            Assets, adversaries, and mitigations
.github/workflows/ci.yml        Formatting, lint, and host tests
```

## Development

The crate versions are pinned to Anchor `1.1.2`. The checked-in program ID is
Anchor's development placeholder and **must** be replaced with a governed
upgrade-authority key before any deployment.

```bash
anchor keys sync
anchor build
cargo test --workspace
```

Do not treat a successful build as an audit. A deployable release requires the
controls listed in [SECURITY.md](SECURITY.md) and the open hardening work in
[issue #3](https://github.com/openpacksduel/escrow/issues/3).

## Design documents

- [Protocol and state machine](docs/protocol.md)
- [Threat model](docs/threat-model.md)
- [Security policy](SECURITY.md)

## License

[MIT](LICENSE)
