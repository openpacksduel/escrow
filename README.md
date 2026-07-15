# OpenPacks Duel Escrow

Open-source Solana escrow for two-player pack-opening duels.

This repository is the public protocol boundary for OpenPacks Duel. The web app,
matchmaking service, and pack-provider adapter live separately in
[`openpacksduel/app`](https://github.com/openpacksduel/app).

> [!WARNING]
> This is an unaudited **devnet MVP**, not a production deployment. It deliberately
> accepts only zero-decimal, single-supply assets owned by the legacy SPL Token
> Program. Programmable NFTs, compressed NFTs, Token-2022 assets, and mainnet
> value are unsupported.

## Current contract

- A creator opens either a direct challenge or an open match.
- Both players deposit the same disclosed platform-fee amount in legacy wrapped
  SOL into a PDA vault. The canonical legacy WSOL mint is enforced on-chain;
  pack purchases are external and never enter this vault.
- An open match binds its opponent when the first non-creator deposits.
- A creator can cancel only before an opponent has joined.
- After the deadline, anyone can trigger a refund to either player's owned token
  account, so neither player depends on an operator.
- Each card is deposited into an isolated PDA-controlled legacy SPL token vault.
  It must have zero decimals, supply one, and permanently revoked mint and freeze
  authorities before custody accepts it.
- The configured provider signs one immutable result commitment binding the duel,
  participants, both card mints, both integer values, and the precommitted
  valuation-policy hash.
- A globally unique provider request ID creates a replay receipt PDA.
- Anyone may settle: the higher committed value receives both card assets and
  both fee deposits go to the committed platform recipient. A tie returns each
  original card and fee deposit without charging the platform fee.
- Before a provider result is committed, expiry recovery is permissionless and
  returns every payment/card deposit to its bound participant.
- Once tracked custody has left a vault, anyone can close it. The payment vault
  first synchronizes raw SOL, then all untracked excess sweeps to the
  precommitted fee recipient. Any NFT sent back to a terminal card vault returns
  to its persisted beneficiary: the original role player after refund/tie, or
  the winner after a non-tie settlement. Rent returns to the creator for the
  payment vault and to the signer that paid to create each card vault. Duel and
  result receipts deliberately remain open so a closed vault
  cannot erase replay protection or settlement history.

The devnet program address is
`Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS`. The planned deployment authority
is `Hk2BD9SiMsePPgbiX85BDuZRX9BbVsde7sdYR7RYgZVo`; its key material is not stored
in this repository. The checksummed build and guarded manual deployment path are
documented in [the devnet release runbook](docs/devnet-deployment.md). Deployment
remains pending a funded devnet authority.

## Repository layout

```text
programs/openpacksduel-escrow/  Anchor program
clients/typescript/             Checked IDL and devnet TypeScript client
docs/protocol.md                State machine and trust boundaries
docs/threat-model.md            Assets, adversaries, and mitigations
.github/workflows/ci.yml        Formatting, lint, and host tests
.github/workflows/program-release.yml  Checksummed program build and devnet deploy
```

## Development

The crate versions are pinned to Anchor `1.1.2`. `Anchor.toml` defaults to
devnet; pass an explicit local validator configuration when developing locally.

```bash
anchor keys sync
anchor build
cargo test --workspace
```

Do not treat a successful build as an audit. A deployable release requires the
controls listed in [SECURITY.md](SECURITY.md) and the open hardening work in
[issue #3](https://github.com/openpacksduel/escrow/issues/3).

App, API, MCP, and agent integrations should use the checked builders in
[`clients/typescript`](clients/typescript) instead of hand-rolling Anchor
instruction bytes or PDA seeds. The package fixes the current canonical program
ID and devnet WSOL payment mint, returns unsigned instruction plans, and rejects
unsupported card standards. Its IDL provenance is tied to source SHA
`4aa3bb7560443c0565ded2d6edee67c6a544dd5f` and workflow run
[`29446296348`](https://github.com/openpacksduel/escrow/actions/runs/29446296348).
That checked client remains pinned to the last checksummed release until a new
program artifact and IDL are produced; newly added source instructions must not
be hand-encoded before that release update.

## Design documents

- [Protocol and state machine](docs/protocol.md)
- [Threat model](docs/threat-model.md)
- [Devnet program release](docs/devnet-deployment.md)
- [Security policy](SECURITY.md)

## License

[MIT](LICENSE)
