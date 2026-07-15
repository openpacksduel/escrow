# Security policy

## Status

This repository is pre-audit and must not be used to custody production value.

## Reporting a vulnerability

Do not open a public issue for an exploitable vulnerability. Use the repository's
**Security > Report a vulnerability** flow with:

- affected commit and instruction;
- impact and required preconditions;
- minimal reproduction or transaction trace;
- suggested mitigation, if known.

Do not test against mainnet accounts or funds you do not own. A public bug-bounty
program and response SLA will be published before mainnet launch.

## Mainnet release gates

- governed upgrade authority (multisig plus timelock);
- program ID and release commit published in advance;
- reproducible/verified build evidence;
- independent Solana-program audit with remediations disclosed;
- provider-attestation test vectors and replay tests;
- adversarial tests for every refund, tie, timeout, and settlement branch;
- emergency pause limited so it cannot block user refunds;
- monitored vault invariants and incident runbook;
- funded public bug bounty.
