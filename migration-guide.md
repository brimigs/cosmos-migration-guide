---
title: Cosmos Migration Guide
description: Learn how to migrate a Cosmos app chain to Solana.
---

This guide covers the process to fully migrate a Cosmos app chain to Solana, including governance, state migration, and token migration.

## Overview

Migrating a Cosmos app chain to Solana involves several key phases:

1. **Preparation** - Governance, development, and testing
2. **Cosmos Wind-Down** - Chain shutdown allowing users to unwind positions
3. **State Processing** - Snapshot export and merkleization of data
4. **Solana Deployment** - Programs, tokens, and governance
5. **Migration Execution** - Token claims and liquidity bootstrapping

---

## Complete Migration Checklist

### Phase 1: Cosmos Wind-Down

- [ ] **Create a Governance Proposal** including the following
  - Snapshot height for chain upgrade to except no new state changes
  - The programs will be deployed on Solana following the chain halt at <PROGRAM_IDS>
  - The chain will be fully decommissioned after the Solana migration is live
  - All governance will migrate to Solana Realms
  - Outline a plan for validator migration and token allocations
  - State the SPL token claim time window
  - State the dates users will be able to unwind positions (if applicable)
- [ ] **Deploy Cosmos SDK upgrade** enabling a "sunset mode"
  - Disable new deposits, staking, and IBC transfers IN
  - Allow withdrawals, unstaking, and IBC transfers OUT
  - Define what "unwind" means for your protocol (LP positions, staked tokens, locked governance tokens)
- [ ] **Grace period** for users to unwind positions
- [ ] **Final Cosmos chain halt** at predetermined height H
  - Coordinate exact halt height with validators
  - Publish final block hash and state root
- [ ] **Export and publish state snapshot** with verification hash
  - Use `app.ExportAppStateAndValidators()` at halt height
  - Make raw snapshot data publicly available

### Phase 2: State Processing

- [ ] **Process snapshot into merkle trees** - Create separate trees for:
  - Token balances (address → amount)
  - Vesting schedules (address → amount, start_time, end_time, cliff)
  - Staking rewards (address → unclaimed_rewards)
  - Governance voting power (address → voting_power)
  - Protocol-specific state (LP positions, loans, NFT ownership, etc.)
    Note: Pick one hash function, SHA-256 is supported natively on Solana.
- [ ] **Publish merkle tree data** to permanent storage (IPFS/Arweave)
- [ ] **Generate and verify merkle roots**

### Phase 3: Solana Deployment

- [ ] **Create SPL token mint(s)**
  - Choose Token Program (legacy) or Token-2022 based on needs
  - Match decimals from Cosmos
  - Set mint authority to claim program initially
- [ ] **Deploy a token claim program** with merkle roots stored in PDAs
  - Implement merkle proof verification
  - Track claims via bitmap or individual PDAs to prevent double-claims
  - Include claim deadline if applicable
- [ ] **Initialize migration config** with roots and parameters
- [ ] **Set up Realms governance structure**
  - Configure community token voting
  - Set voting thresholds and proposal requirements
- [ ] **Create new accounts for on-chain state**
  - Accounts will need to be created to hold custom on-chain state, i.e. vesting schedules
- [ ] **Deploy protocol-specific programs** (AMM, lending, staking, etc.)

### Phase 4: Migration Execution

- [ ] **Open claims** - Consider phased rollout
- [ ] **Migrate vesting schedules** to Solana vesting contracts
- [ ] **Bootstrap initial liquidity** on DEXs
- [ ] **Transfer program authorities** to governance

### Phase 5: Post-Migration

- [ ] **Monitor claims** and provide user support
- [ ] **Handle unclaimed tokens** at deadline (treasury, burn, or redistribute)
- [ ] **Sunset Cosmos archive nodes** (keep some running for historical queries)
- [ ] **Final security audit** of live system

---

## Address Linking

Since Cosmos uses bech32 addresses and Solana uses base58, users need a way to prove ownership of their Solana address.

### Recommended Approach: Pre-Registration + Signature Fallback

**Primary (Pre-Registration)**: Before the snapshot, users submit an on-chain transaction on Cosmos that registers their Solana address.

- Cleanest UX at claim time
- On-chain proof eliminates disputes

**Fallback (Signature Proof)**: Users who missed registration can sign a message with their Cosmos private key (secp256k1) proving ownership of their Solana address.

- Use Solana's `secp256k1_recover` for verification
- Requires users to still have access to Cosmos keys

---

## Token Claim Program

The claim program is the critical component that distributes tokens to users based on their Cosmos holdings.

### Key Components

- **Merkle root storage**: Store roots in a PDA owned by the claim program
- **Proof verification**: Verify user-provided merkle proofs on-chain
- **Claim tracking**: Bitmap or individual claim PDAs to prevent double-claims
- **Events**: Emit events for all claims (enables indexing and monitoring)

### Reference Implementations

- [Jito Foundation Distributor](https://github.com/jito-foundation/distributor) - Merkle-based distribution with vesting support
- [Metaplex Gumdrop](https://developers.metaplex.com/guides/general/spl-token-claim-airdrop-using-gumdrop) - Claimable SPL token airdrops

---

## Governance Migration

Use [SPL Governance (Realms)](https://realms.today) for on-chain governance on Solana.

### Configuration

- **Community token**: Your migrated governance token
- **Council token** (optional): For core team/foundation multisig
- **Voting thresholds**: Match or adjust from Cosmos governance parameters
- **Proposal requirements**: Deposit amounts, voting periods

### Vesting Integration

Options for migrating vesting schedules:

- **Realms built-in vesting**: Simpler but less flexible
- **Custom vesting program**: More control, integrates with Realms for voting
- **[Bonfida Token Vesting](https://github.com/Bonfida/token-vesting)**: Established solution

---

## Complex State Migration

### DeFi Positions

**Liquidity Positions (AMM)**:

- Snapshot LP token balances and underlying asset ratios
- Option: Migrate LP tokens as-is, or break down into underlying assets

**Lending/Borrowing**:

- Snapshot collateral, debt, and health factors
- Handle bad debt before migration to avoid porting insolvency

**Staking/Delegation**:

- Map to new staking structure
- Credit already-unbonding amounts appropriately

### NFT Migration

- Snapshot all NFT ownership (token IDs → owners)
- Redeploy collection metadata using Metaplex standard
- Create claim mechanism for each NFT
- Choose between compressed NFTs (cheaper) vs standard NFTs (more compatible)

---

## Architecture Differences: Cosmos vs Solana

| Cosmos                      | Solana                                 |
| --------------------------- | -------------------------------------- |
| Messages                    | Instructions                           |
| Module state (single store) | Accounts (separate PDAs)               |
| Synchronous execution       | Account-based parallelism              |
| Gas metering                | Compute units (200k default, 1.4M max) |
| CosmWasm contracts          | Rust programs (BPF bytecode)           |

### Solana Constraints to Consider

- Account size limit: 10MB max (costs rent)
- Transaction size limit: 1232 bytes
- CPI depth limit: 4 levels
- No native floating point

---

## Resources

- [SPL Governance (Realms)](https://realms.today) - DAO governance
- [Cosmos SDK Snapshots](https://pkg.go.dev/github.com/cosmos/cosmos-sdk/snapshots) - State export
- [Solana Token Program](https://www.solana-program.com/docs/token) - SPL tokens
- [Token-2022 Extensions](https://www.solana-program.com/docs/token-2022/extensions) - Advanced token features
- [Helius Airdrop Guide](https://www.helius.dev/blog/solana-airdrop) - Efficient distribution methods
- [Example Basic Token Vesting](https://github.com/solana-developers/developer-bootcamp-2024/tree/main/project-8-token-vesting) - Vesting contract example
