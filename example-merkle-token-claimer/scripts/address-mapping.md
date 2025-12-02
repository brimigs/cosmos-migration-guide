# Address Mapping: Cosmos to Solana

Users need to link their Cosmos address to a Solana address before claiming tokens. This document covers two approaches.

## Approach 1: Pre-Registration (Recommended)

Users register their Solana address on-chain before the snapshot.

### How It Works

1. Deploy a simple registration contract on Cosmos before the snapshot date
2. Users submit a transaction: `RegisterSolanaAddress { solana_address: "base58..." }`
3. At snapshot, export all registrations alongside token balances
4. Build merkle tree using registered Solana addresses

### Benefits
- No cryptographic verification needed at claim time
- Users can claim immediately when migration goes live
- On-chain proof eliminates disputes

### Example Registration Message (CosmWasm)

```rust
#[cw_serde]
pub enum ExecuteMsg {
    RegisterSolanaAddress { solana_address: String },
}
```

### Example Export Query

```bash
# Export all registrations at snapshot height
gaiad query wasm contract-state all <contract_addr> --height <snapshot_height>
```

---

## Approach 2: Signature Fallback

For users who missed pre-registration, allow them to prove ownership by signing with their Cosmos key.

### How It Works

1. User signs a message with their Cosmos private key (secp256k1)
2. Message format: `"Claim Solana tokens to: <solana_address>"`
3. Verification uses Solana's `secp256k1_recover` precompile
4. If signature valid, map Cosmos address → Solana address

### On-Chain Verification (Solana)

```rust
use solana_program::secp256k1_recover::secp256k1_recover;

// Recover the Cosmos public key from the signature
let recovered_pubkey = secp256k1_recover(
    &message_hash,
    recovery_id,
    &signature,
)?;

// Derive Cosmos address from recovered pubkey
let cosmos_address = derive_cosmos_address(&recovered_pubkey);

// Verify it matches the expected Cosmos address in the merkle leaf
require!(cosmos_address == expected_cosmos_address);
```

### Considerations
- Requires users to still have access to their Cosmos keys
- More complex claim UX (signing step)
- Additional compute cost for signature verification

---

## Data Schema

### Snapshot with Pre-Registration

```json
{
  "entries": [
    {
      "cosmos_address": "cosmos1abc...",
      "solana_address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
      "amount": 1000000000,
      "source": "pre_registration"
    }
  ]
}
```

### Snapshot without Registration (Signature Fallback)

```json
{
  "entries": [
    {
      "cosmos_address": "cosmos1xyz...",
      "cosmos_pubkey": "02abc123...",
      "amount": 500000000,
      "source": "signature_fallback"
    }
  ]
}
```

For signature fallback entries, users provide their Solana address + signature at claim time.

---

## Recommended Flow

```
┌─────────────────────────────────────────────────────────┐
│  Before Snapshot                                        │
│  ─────────────────                                      │
│  1. Deploy registration contract on Cosmos              │
│  2. Announce deadline for pre-registration              │
│  3. Users register: cosmos_addr → solana_addr           │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  At Snapshot                                            │
│  ───────────                                            │
│  1. Export token balances                               │
│  2. Export address registrations                        │
│  3. Merge into final snapshot                           │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Post-Migration                                         │
│  ──────────────                                         │
│  Pre-registered users: Claim directly with proof        │
│  Unregistered users: Sign message + claim with proof    │
└─────────────────────────────────────────────────────────┘
```

## Security Notes

- Validate Solana addresses are valid base58 during registration
- Consider replay protection for signature fallback (include chain ID, nonce)
- Set a claim deadline to limit exposure window
