# Merkle Token Claimer for Cosmos Migration

This example demonstrates a realistic Merkle-based token migration flow on Solana. Users with balances on a deprecated Cosmos chain can claim equivalent SPL tokens on Solana by presenting a stable Merkle proof, and the program prevents double-claims with dedicated claim receipt PDAs.

The Anchor workspace in this example is configured for Anchor `1.0.0`.

## How It Works

1. **Snapshot Cosmos balances** at a chosen height and map each Cosmos owner to a Solana address.
2. **Build a fixed Merkle tree** where each leaf contains `[solana_pubkey (32 bytes) | amount (8 bytes)]`.
3. **Initialize the airdrop** by storing the Merkle root on-chain and minting the full claimable supply into a vault ATA owned by the program PDA.
4. **Users claim independently** by submitting `amount + merkle_proof + index`.
5. **Program creates a claim receipt PDA** for that index, preventing the same allocation from being claimed twice.

## What You Need to Add

### 1. Cosmos Integration (Off-Chain)

You must build tooling to:

- Query your Cosmos chain for all token holder balances at a specific block height
- Map Cosmos addresses to Solana addresses before the snapshot
- Generate the Merkle tree from the finalized snapshot data
- Provide a frontend or API for users to fetch their Merkle proof and index

### 2. Token Configuration

Update the following for your migration:

| Item | Location | Description |
|------|----------|-------------|
| Program ID | `Anchor.toml`, `lib.rs` | Deploy your own program instance |
| Token Mint | Initialization call | Your new Solana SPL token mint address |
| Merkle Root | Generated from snapshot | 32-byte hash of your balance tree |
| Total Amount | Initialization call | Sum of all claimable balances |

## Key Files

```text
programs/merkle-tree-token-claimer/src/lib.rs  # On-chain program
programs/merkle-tree-token-claimer/tests/merkle_tree_token_claimer_litesvm.rs  # LiteSVM integration coverage
scripts/generate-merkle-tree.ts                # Off-chain tree/proof generator
```

## Program Instructions

| Instruction | Purpose | Who Calls |
|-------------|---------|-----------|
| `initialize_airdrop_data` | Create state, mint, and vault funding | Authority (once) |
| `update_tree` | Replace the Merkle root before any claims happen | Authority only |
| `claim_airdrop` | Verify proof, transfer tokens, and create receipt PDA | Whitelisted user |

## Claim Flow

```text
User submits: amount + merkle_proof + index
    ↓
Program recomputes the leaf hash from signer + amount
    ↓
Program verifies the proof against the fixed on-chain root
    ↓
Program creates claim_receipt PDA for that index
    ↓
Tokens transfer from vault → user's ATA
```

Unlike the earlier mutable-root approach, one user claiming does not invalidate other users' proofs.

## Building the Merkle Tree

The tree uses **SHA-256 hashing**. Each leaf must be exactly 40 bytes:

```text
bytes 0-31: Solana pubkey
bytes 32-39: Amount (u64, little-endian)
```

Use the `svm-merkle-tree` library to construct the tree and generate proofs client-side.

## Running Tests

```bash
anchor build --ignore-keys
cargo test -p merkle-tree-token-claimer --test merkle_tree_token_claimer_litesvm -- --nocapture
```

The LiteSVM test loads `target/deploy/merkle_tree_token_claimer.so` directly, so the program must be built before running the Rust integration test.

## Security Notes

- Mint authority is revoked after initialization, so no extra tokens can be minted later.
- Claim proofs are stable because the Merkle root does not change after users start claiming.
- Double-claims are blocked by `claim_receipt` PDAs derived from `(airdrop_state, index)`.
- `update_tree` is only allowed before the first claim. If you need to change both the root and total funded amount after launch, redeploy a new distribution instance instead of mutating a live one.
