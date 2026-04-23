# Merkle Tree Scripts

Scripts to generate Merkle roots and proofs from Cosmos migration snapshots.

## Prerequisites

Run from the parent directory (`example-merkle-token-claimer`):

```bash
npm install
```

## Snapshot Format

Input JSON maps Cosmos addresses to Solana addresses and uses string amounts in the smallest unit:

```json
{
  "snapshot_height": 12345678,
  "chain_id": "your-chain-id",
  "timestamp": "2024-01-15T00:00:00Z",
  "entries": [
    {
      "cosmos_address": "cosmos1abc...",
      "solana_address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
      "amount": "1000000000"
    }
  ]
}
```

String amounts avoid JavaScript safe-integer issues for large token balances.

## Generate Merkle Tree

```bash
cd scripts
npx ts-node generate-merkle-tree.ts sample-snapshot.json output.json
```

## Output Format

The script generates:

```json
{
  "merkle_root": [1, 2, 3, "..."],
  "merkle_root_hex": "0102030405...",
  "total_amount": "1750000000",
  "total_entries": 3,
  "snapshot_height": 12345678,
  "proofs": [
    {
      "solana_address": "7xKXtg...",
      "cosmos_address": "cosmos1abc...",
      "amount": "1000000000",
      "index": 0,
      "proof": "deadbeef..."
    }
  ]
}
```

## Using the Output

### On-Chain Initialization

Pass the Merkle root and total amount to `initialize_airdrop_data`:

```typescript
await program.methods
  .initializeAirdropData(output.merkle_root, new BN(output.total_amount))
  .accounts({ ... })
  .rpc();
```

### User Claims

Convert proof hex back to bytes for `claim_airdrop`:

```typescript
const proofBytes = Buffer.from(userProof.proof, "hex");
await program.methods
  .claimAirdrop(new BN(userProof.amount), proofBytes, new BN(userProof.index))
  .accounts({ ... })
  .rpc();
```

## Hashing Algorithm

Uses **SHA-256**.

## Leaf Format

Each leaf is 40 bytes:

- Bytes 0-31: Solana public key
- Bytes 32-39: Amount (`u64`, little-endian)

This matches the on-chain program's expected format.
