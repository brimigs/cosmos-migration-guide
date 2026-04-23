# Cosmos Migration Guide Workspace

This repository is a small workspace of example programs and reference material for migrating application patterns from Cosmos-style contracts to Solana programs.

It currently contains two main example sets:

- `example-escrow-contracts/`
  Side-by-side receipt-based escrow examples in:
  - CosmWasm
  - Anchor (Solana)
  - Pinocchio (Solana)
- `example-merkle-token-claimer/`
  A Solana Merkle-based token migration flow for distributing replacement SPL balances to users from a finalized Cosmos snapshot.

## Workspace Layout

```text
example-escrow-contracts/
  README.md
  anchor/
  cosmwasm/
  pinocchio/

example-merkle-token-claimer/
  README.md
  programs/merkle-tree-token-claimer/
  programs/merkle-tree-token-claimer/tests/
  scripts/
```

## What Each Example Covers

### `example-escrow-contracts`

This section compares the same high-level escrow lifecycle across different runtimes:

`create escrow` -> `allow mint/token` -> `deposit` -> `withdraw`

Use it when you want to understand:

- how CosmWasm storage maps translate into Solana PDAs or raw accounts
- how token custody differs between CW20 contracts and SPL token vaults
- how receipt-based deposits scale better than one-off escrow deals

Notes:

- The CosmWasm example now validates external addresses with `deps.api.addr_validate(...)`.
- The Anchor example has been updated to Anchor `1.0.0`.
- The Pinocchio example is included for architectural comparison.

See [example-escrow-contracts/README.md](/Users/brimigs/code/cosmos-migration-guide/example-escrow-contracts/README.md) for the detailed comparison.

### `example-merkle-token-claimer`

This section models a token migration where users prove an allocation from a fixed Merkle tree and claim replacement SPL tokens on Solana.

Use it when you need to migrate balances from a deprecated Cosmos chain by:

- snapshotting balances off-chain
- mapping Cosmos users to Solana addresses
- generating Merkle proofs
- preventing double claims with receipt PDAs

See [example-merkle-token-claimer/README.md](/Users/brimigs/code/cosmos-migration-guide/example-merkle-token-claimer/README.md) for the full flow and test instructions.

## Tooling Notes

These examples are not a single unified Cargo or Anchor workspace. Each subdirectory should be treated independently.

Current toolchain expectations in this repo:

- `example-escrow-contracts/anchor` uses Anchor `1.0.0`
- `example-merkle-token-claimer` currently uses Anchor `0.30.1`
- `example-escrow-contracts/cosmwasm` is a standalone Rust/CosmWasm crate
- `example-escrow-contracts/pinocchio` is a standalone Rust/Pinocchio crate

Because of that, switch toolchains per example instead of assuming one global Anchor version for the whole repository.

## Getting Started

Pick the example you want to work on, then use its local README and manifest files as the source of truth.

Common entry points:

- Escrow comparison: `cd example-escrow-contracts`
- Merkle migration example: `cd example-merkle-token-claimer`

Useful starting commands:

```bash
# CosmWasm escrow
cd example-escrow-contracts/cosmwasm
cargo check

# Anchor escrow
cd example-escrow-contracts/anchor
cargo check

# Merkle claimer
cd example-merkle-token-claimer
anchor build --ignore-keys
```

## Git Hygiene

The repo root `.gitignore` currently ignores generated build output for the escrow examples:

- `example-escrow-contracts/anchor/target/`
- `example-escrow-contracts/cosmwasm/target/`
- `example-escrow-contracts/pinocchio/target/`

If you add more generated artifacts at the workspace level, extend `.gitignore` accordingly.
