# Devnet program release

The `Program release` GitHub Actions workflow builds a deployable Anchor program
on every relevant pull request and `main` update. A manual run can deploy that
exact artifact to Solana devnet after an explicit target confirmation.

This path is devnet-only. It does not authorize a mainnet deployment and does
not change the program's unaudited status.

## Fixed identities

| Role | Public key |
| --- | --- |
| Program | `Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS` |
| Upgrade and deployment authority | `Hk2BD9SiMsePPgbiX85BDuZRX9BbVsde7sdYR7RYgZVo` |

The workflow rejects a keypair whose derived public key does not match the
expected identity. It also checks the devnet genesis hash before loading or
using either key.

## GitHub environment and secrets

Create a GitHub environment named `devnet`. Add a required reviewer if the
repository plan supports environment protection, then define these environment
secrets:

- `SOLANA_DEVNET_DEPLOY_AUTHORITY_KEYPAIR_JSON`: the JSON byte array for the
  expected deployment authority. It is required for initial deployments and
  upgrades.
- `SOLANA_DEVNET_PROGRAM_KEYPAIR_JSON`: the JSON byte array whose public key is
  the reserved program ID. It is required only for the initial deployment. The
  workflow uses the public program address for later upgrades.

Never paste either key into an issue, log, workflow input, artifact, repository
variable, or tracked file. Rotate the authority after the MVP if this CI-based
signing model is retained; a multisig or governed authority is required before
mainnet consideration.

## Funding

The authority must receive devnet SOL before the first run. A new upgradeable
program temporarily needs both a program-sized buffer and its persistent
ProgramData allocation, plus transaction fees. The workflow computes a safety
floor from the compiled byte length:

```text
2 × devnet rent(program artifact bytes) + 0.01 SOL
```

This is a one-time devnet funding requirement for the initial deployment. Keep
a smaller balance available for later upgrade buffers and transaction fees. The
workflow stops before deployment and prints the required lamports when the
authority is underfunded.

## Build artifact

The build job pins and verifies the download checksums for Anchor `1.1.2` and
Agave `3.1.10`, and uses Rust `1.89.0`. Its artifact contains only public build
outputs:

- `openpacksduel_escrow.so`
- `openpacksduel_escrow.json` (Anchor IDL)
- `source-Cargo.lock`
- `build-manifest.json`
- `SHA256SUMS`

The generated program keypair under `target/deploy` is deliberately excluded.
The source commit, program ID, toolchain versions, file sizes, and hashes are
recorded in the manifest.

## Manual deploy

1. Open **Actions → Program release → Run workflow**.
2. Select the exact branch or commit intended for devnet.
3. Set `deploy` to `true`.
4. Enter
   `devnet:Co198eFfQcmn1WzZRnHV6jxcSLBDCv1qNfPfiBYdCLfS` in the confirmation
   field.
5. Approve the `devnet` environment gate when configured.

The deploy job verifies the artifact checksums, source SHA, devnet genesis hash,
authority, reserved program ID, upgradeable-loader owner, and executable program
metadata. It then dumps the on-chain program and requires its SHA-256 hash to
match the artifact before uploading a public verification receipt.
