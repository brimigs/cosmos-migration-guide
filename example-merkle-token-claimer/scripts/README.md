# Merkle Tree Scripts

Scripts to generate merkle trees from Cosmos chain snapshots.

## Prerequisites

Run from the parent directory (`example-merkle-token-claimer`):
```bash
npm install
```

## Snapshot Format

Input JSON must contain an array of entries mapping Cosmos addresses to Solana addresses:

```json
{
  "snapshot_height": 12345678,
  "chain_id": "your-chain-id",
  "timestamp": "2024-01-15T00:00:00Z",
  "entries": [
    {
      "cosmos_address": "cosmos1abc...",
      "solana_address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
      "amount": 1000000000
    }
  ]
}
```

**Note**: Amounts should be in the smallest unit (e.g., uatom, not ATOM).

## Generate Merkle Tree

```bash
cd scripts
npx ts-node generate-merkle-tree.ts sample-snapshot.json output.json
```

## Output Format

The script generates:

```json
{
  "merkle_root": [1, 2, 3, ...],        // 32-byte array for on-chain init
  "merkle_root_hex": "0102030405...",   // Hex string for verification
  "total_amount": 1750000000,           // Sum of all amounts
  "total_entries": 3,
  "snapshot_height": 12345678,
  "proofs": [
    {
      "solana_address": "7xKXtg...",
      "cosmos_address": "cosmos1abc...",
      "amount": 1000000000,
      "index": 0,
      "proof": "deadbeef..."           // Hex-encoded proof bytes
    }
  ]
}
```

## Using the Output

### On-Chain Initialization

Pass `merkle_root` array to `initialize_airdrop_data`:

```typescript
await program.methods
  .initializeAirdropData(output.merkle_root, new BN(output.total_amount))
  .accounts({...})
  .rpc();
```

### User Claims

Convert proof hex back to bytes for claim instruction:

```typescript
const proofBytes = Buffer.from(userProof.proof, "hex");
await program.methods
  .claimAirdrop(new BN(userProof.amount), proofBytes, new BN(userProof.index))
  .accounts({...})
  .rpc();
```

## Hashing Algorithm

Uses **SHA-256** which is natively supported on Solana (more compute-efficient than Keccak).

## Leaf Format

Each leaf is 41 bytes:
- Bytes 0-31: Solana public key
- Bytes 32-39: Amount (u64, little-endian)
- Byte 40: Claimed flag (0 = unclaimed)

This matches the on-chain program's expected format.
