# Merkle Token Claimer for Cosmos Migration

This example demonstrates a merkle tree-based token claiming mechanism on Solana. Users with balances on your deprecated Cosmos chain can claim equivalent tokens on Solana by providing a merkle proof.

## How It Works

1. **Snapshot Cosmos balances** - Export all holder addresses and balances from your Cosmos chain
2. **Build merkle tree** - Each leaf contains: `[solana_pubkey (32 bytes) | amount (8 bytes) | claimed_flag (1 byte)]`
3. **Initialize airdrop** - Deploy the program with the merkle root and mint total supply to a vault
4. **Users claim** - Each user provides their merkle proof to claim their tokens

## What You Need to Add

### 1. Cosmos Integration (Off-Chain)

You must build tooling to:

- Query your Cosmos chain for all token holder balances at a specific block height
- Map Cosmos addresses to Solana addresses (users must register their Solana pubkey)
- Generate the merkle tree from the snapshot data
- Provide a frontend/API for users to fetch their merkle proofs

### 2. Token Configuration

Update the following for your migration:

| Item | Location | Description |
|------|----------|-------------|
| Program ID | `Anchor.toml`, `lib.rs` | Deploy your own program instance |
| Token Mint | Initialization call | Your new Solana SPL token mint address |
| Merkle Root | Generated from snapshot | 32-byte hash of your balance tree |
| Total Amount | Initialization call | Sum of all claimable balances |

## Key Files

```
programs/merkle-tree-token-claimer/src/lib.rs  # On-chain program (3 instructions)
tests/merkle-tree-token-claimer.ts             # Example usage and tests
```

## Program Instructions

| Instruction | Purpose | Who Calls |
|-------------|---------|-----------|
| `initialize_airdrop_data` | Create vault, set merkle root, mint tokens | Authority (once) |
| `update_tree` | Update merkle root if needed | Authority only |
| `claim_airdrop` | Claim tokens with merkle proof | Any whitelisted user |

## Claim Flow

```
User submits: amount + merkle_proof + index
    ↓
Program verifies proof against on-chain merkle root
    ↓
Tokens transfer from vault → user's token account
    ↓
Merkle root updates (marks leaf as claimed, prevents double-claim)
```

## Building the Merkle Tree

The tree uses **SHA-256 hashing** (native on Solana, more compute-efficient). Each leaf must be exactly 41 bytes:

```
bytes 0-31:  Solana pubkey (the claimer's address)
bytes 32-39: Amount (u64, little-endian)
byte 40:     Claimed flag (0 = unclaimed, 1 = claimed)
```

Use the `svm-merkle-tree` library (see `package.json` devDependencies) to construct trees and generate proofs client-side.

## Running Tests

```bash
anchor build
anchor test
```

## Security Notes

- Mint authority is revoked after initialization (no additional minting possible)
- Only the program PDA can transfer from the vault
- Merkle root changes after each claim, invalidating reused proofs
- Authority can update the tree before claims if corrections are needed
