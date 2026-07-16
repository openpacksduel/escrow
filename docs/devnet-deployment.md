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
- `SOLANA_DEVNET_BUFFER_KEYPAIR_JSON`: a dedicated JSON keypair byte array for
  the reusable deployment buffer. It is required for every deployment. The
  workflow always passes this keypair with `solana program deploy --buffer`, so
  the CLI never generates a random buffer keypair or prints its recovery phrase.
  If a deploy is interrupted, the next run with the same secret resumes the
  existing buffer instead of abandoning its funded writes.
- `SOLANA_DEVNET_PROGRAM_KEYPAIR_JSON`: the JSON byte array whose public key is
  the reserved program ID. It is required only for the initial deployment. The
  workflow uses the public program address for later upgrades.

Generate the buffer keypair once on a trusted machine without printing a
recovery phrase, store it as the `devnet` environment secret, and remove the
temporary file:

```bash
(
  set -euo pipefail
  umask 077
  buffer_keypair_path=./devnet-buffer.keypair.json
  trap 'rm -f "$buffer_keypair_path"' EXIT

  solana-keygen new \
    --silent \
    --no-bip39-passphrase \
    --outfile "$buffer_keypair_path"
  gh secret set SOLANA_DEVNET_BUFFER_KEYPAIR_JSON \
    --repo openpacksduel/escrow \
    --env devnet \
    < "$buffer_keypair_path"
)
```

Do not reuse the deployment authority or program keypair as the buffer keypair.
Never paste any keypair into an issue, log, workflow input, artifact, repository
variable, or tracked file. Rotate the authority after the MVP if this CI-based
signing model is retained; a multisig or governed authority is required before
mainnet consideration.

The buffer account's public address is safe to inspect and may appear in the
deployment verification receipt. Its keypair JSON must remain secret. A
successful deployment consumes and closes the buffer account; the same keypair
can be used for a later release. A failed deployment may leave the buffer
account funded and partially written, which is expected: do not close it before
retrying the exact workflow ref.

## Funding

The authority must receive devnet SOL before the first run. A new upgradeable
program temporarily needs both a program-sized buffer and its persistent
ProgramData allocation, plus transaction fees. An upgrade only needs the buffer
and fees. The workflow computes a safety floor from the compiled byte length and
any balance already held by the configured buffer:

```text
missing buffer rent
+ initial ProgramData rent, for the first deployment only
+ 0.01 SOL for deployment fees
```

This is a one-time devnet funding requirement for the initial deployment. Keep
a smaller balance available for later upgrade buffers and transaction fees. The
workflow stops before deployment and prints the required lamports when the
authority is underfunded. If a failed run created the configured buffer, leave
its SOL in place: the next run detects and reuses that account.

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

## First-deployment compatibility gate

A read-only `getAccountInfo` query against the canonical Solana devnet RPC on
2026-07-16 returned no account for the reserved program ID. No earlier program
deployment or v2 Duel account state exists at that address, so v4 can be the
first deployment without an account migration.

This observation is not permanent evidence. Immediately before the first
deployment, re-run the program-account lookup and require the address to remain
absent. If it exists, stop: do not deploy or document compatibility until its
owner, executable metadata, deployed bytes, upgrade authority, and any existing
Duel accounts have been audited. The v4 Duel account is 560 bytes and is not
compatible with the former 432-byte layout.

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

If the Solana CLI fails, the workflow deliberately suppresses its raw stdout and
stderr. Some CLI deployment failure paths can include key-recovery material.
The Actions log instead reports a generic failure and instructs the operator to
rerun the same ref. Use read-only RPC inspection of the public buffer address
for deeper diagnosis; never copy raw key recovery output into public logs.
