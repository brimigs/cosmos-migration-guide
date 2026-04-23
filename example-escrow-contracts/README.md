# Receipt Escrow Contract: CosmWasm vs Anchor vs Pinocchio

Side-by-side comparison of a receipt-based escrow flow implemented in CosmWasm (Cosmos), Anchor (Solana), and Pinocchio (Solana). All three examples now follow the same high-level lifecycle inspired by the official [`solana-program/escrow`](https://github.com/solana-program/escrow) repository:

`create escrow` → `allow mint/token` → `deposit` → `withdraw`

## Key Differences

| Aspect | CosmWasm | Anchor (Solana) | Pinocchio (Solana) |
|--------|----------|-----------------|---------------------|
| **Entry points** | Multiple (`instantiate`, `execute`, `query`) | Single `#[program]` module | Single manual entrypoint with discriminators |
| **State storage** | Contract storage (`Item`, `Map`) | PDA accounts | Raw account bytes |
| **Asset custody** | Contract address holds CW20 tokens | Vault ATA owned by escrow PDA | Vault ATA owned by escrow PDA |
| **Allowlist state** | `Map<validated token address, bool>` | `AllowedMint` PDA | `AllowedMintAccount` raw bytes |
| **Deposit tracking** | `Receipt` records in contract storage | `Receipt` PDA per deposit | `ReceiptAccount` raw bytes |
| **Routing** | Enum matching in `execute()` | Separate instruction handlers | Manual instruction parsing |
| **Validation style** | `addr_validate` plus manual sender/storage checks | Declarative account constraints | Explicit signer/owner/writable checks |

## Shared Flow

1. Admin creates the escrow configuration.
2. Admin allowlists a token/mint that may be deposited.
3. A depositor transfers tokens into escrow and receives a receipt keyed by `receipt_seed`.
4. The original depositor later redeems that receipt to withdraw the tokens.

## Code Comparison

### Escrow Configuration

**CosmWasm** - Contract-owned config with validated admin address:
```rust
let admin = deps.api.addr_validate(&msg.admin)?;
ESCROW.save(
    deps.storage,
    &EscrowConfig {
        admin: admin.to_string(),
        escrow_seed: msg.escrow_seed.clone(),
    },
)?;
```

**Anchor** - PDA account:
```rust
#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub version: u8,
    pub bump: u8,
    pub escrow_seed: Pubkey,
    pub admin: Pubkey,
}
```

**Pinocchio** - Raw byte layout:
```rust
#[repr(C)]
pub struct EscrowAccount {
    pub discriminator: u8,
    pub version: u8,
    pub bump: u8,
    pub escrow_seed: [u8; 32],
    pub admin: [u8; 32],
}
```

### Allowed Mint / Token

**CosmWasm** - Storage map keyed by validated token address:
```rust
let token_addr = deps.api.addr_validate(&token_address)?;
ALLOWED_TOKENS.save(deps.storage, token_addr.as_str(), &true)?;
```

**Anchor** - Dedicated PDA:
```rust
#[account]
#[derive(InitSpace)]
pub struct AllowedMint {
    pub version: u8,
    pub bump: u8,
}
```

**Pinocchio** - Minimal raw marker account:
```rust
#[repr(C)]
pub struct AllowedMintAccount {
    pub discriminator: u8,
    pub version: u8,
    pub bump: u8,
}
```

### Deposit Receipt

**CosmWasm** - Stored under `(depositor, receipt_seed)`:
```rust
#[cw_serde]
pub struct Receipt {
    pub depositor: String,
    pub token_address: String,
    pub receipt_seed: String,
    pub amount: Uint128,
    pub deposited_at: u64,
}
```

**Anchor** - PDA receipt:
```rust
#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub version: u8,
    pub bump: u8,
    pub escrow: Pubkey,
    pub depositor: Pubkey,
    pub mint: Pubkey,
    pub receipt_seed: Pubkey,
    pub amount: u64,
    pub deposited_at: i64,
}
```

**Pinocchio** - Raw receipt bytes:
```rust
#[repr(C)]
pub struct ReceiptAccount {
    pub discriminator: u8,
    pub version: u8,
    pub bump: u8,
    pub padding: [u8; 7],
    pub escrow: [u8; 32],
    pub depositor: [u8; 32],
    pub mint: [u8; 32],
    pub receipt_seed: [u8; 32],
    pub amount: u64,
    pub deposited_at: i64,
}
```

### Deposit

**CosmWasm** - Validate the CW20 address, transfer into the contract, and record a receipt:
```rust
ExecuteMsg::Deposit { token_address, receipt_seed, amount } => {
    let token_addr = deps.api.addr_validate(&token_address)?;
    RECEIPTS.save(deps.storage, (depositor.as_str(), receipt_seed.as_str()), &receipt)?;
    Ok(Response::new().add_message(WasmMsg::Execute {
        contract_addr: token_addr.to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom { ... })?,
        funds: vec![],
    }))
}
```

**Anchor** - Transfer SPL tokens into the vault ATA and initialize a `Receipt` PDA:
```rust
pub fn deposit(ctx: Context<Deposit>, receipt_seed: Pubkey, amount: u64) -> Result<()> {
    transfer_checked(...)?;
    ctx.accounts.receipt.amount = amount;
    Ok(())
}
```

**Pinocchio** - Same idea with manual account parsing and raw byte storage:
```rust
fn deposit(program_id: &Address, accounts: &mut [AccountView], instruction_data: &[u8]) -> ProgramResult {
    Transfer::new(depositor_token_account, vault, depositor, amount).invoke()?;
    ReceiptAccount { /* ... */ }.store(receipt)?;
    Ok(())
}
```

### Withdraw

**CosmWasm** - Sender redeems their stored receipt using the same validated token address:
```rust
ExecuteMsg::Withdraw { token_address, receipt_seed } => {
    let token_addr = deps.api.addr_validate(&token_address)?;
    let receipt = RECEIPTS.load(deps.storage, (depositor.as_str(), receipt_seed.as_str()))?;
    if receipt.token_address != token_addr.as_str() {
        return Err(ContractError::ReceiptNotFound);
    }
    RECEIPTS.remove(deps.storage, (depositor.as_str(), receipt_seed.as_str()));
    Ok(Response::new().add_message(WasmMsg::Execute {
        contract_addr: token_addr.to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer { ... })?,
        funds: vec![],
    }))
}
```

**Anchor** - Receipt PDA authorizes the withdrawer’s claim from the escrow vault:
```rust
pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
    transfer_checked(...)?;
    Ok(())
}
```

**Pinocchio** - Escrow PDA signs the token CPI using explicit seeds:
```rust
let signer_seeds = [
    Seed::from(b"escrow".as_slice()),
    Seed::from(escrow_state.escrow_seed.as_slice()),
    Seed::from(bump_ref.as_slice()),
];
let signer = Signer::from(&signer_seeds[..]);

Transfer::new(vault, withdrawer_token_account, escrow, receipt_state.amount)
    .invoke_signed(&[signer])?;
```

## Migration Considerations

1. **Receipts replace single-deal state**: This structure scales better for many independent deposits than a one-off arbiter escrow.
2. **Allowance / custody differs by chain**: CosmWasm uses CW20 `TransferFrom` into the contract; Solana uses token-program CPIs into a PDA-owned vault ATA.
3. **Allowlists become explicit state**: You need either a storage map or a dedicated account/PDA to mark supported assets.
4. **PDAs replace storage keys on Solana**: Escrow, allowlist entries, receipts, and vault accounts all become derived addresses.
5. **CosmWasm should validate external addresses at the boundary**: Normalize admin, token, and query inputs with `deps.api.addr_validate(...)` before storing or comparing them.
6. **Queries remain off-chain on Solana**: Read escrow, receipt, and vault state via RPC instead of on-chain query entry points.

## Building

## Prerequisites

- Rust installed via `rustup`, not Homebrew `rust`
- Solana CLI with SBF tooling available on `PATH`
- Anchor CLI `1.0.x`

On macOS, verify your shell is using rustup-managed Rust before building Solana programs:

```bash
which cargo
which rustc
which rustup
```

`cargo` should resolve to `~/.cargo/bin/cargo`. If it resolves to `/opt/homebrew/bin/cargo`, `anchor build` and `cargo build-sbf` can fail because Homebrew's `cargo` does not support rustup's `cargo +toolchain ...` syntax used by Solana tooling.

## Building

**CosmWasm**:
```bash
cd cosmwasm
cargo build --release --target wasm32-unknown-unknown
```

**Anchor**:
```bash
cd anchor
anchor build
```

**Pinocchio**:
```bash
cd pinocchio
cargo build-sbf --features bpf-entrypoint
```

## Testing

Both Solana implementations now have LiteSVM-based Rust integration tests.

**Anchor**:
```bash
cd anchor
anchor build
cargo test -p escrow --test escrow_litesvm -- --nocapture
```

The Anchor test suite uses `anchor-litesvm` plus `litesvm-utils` and covers:
- successful `create_escrow -> allow_mint -> deposit -> withdraw`
- zero-amount deposit rejection

**Pinocchio**:
```bash
cd pinocchio
cargo build-sbf --features bpf-entrypoint
cargo test --test escrow_litesvm -- --nocapture
```

The Pinocchio test suite uses raw `litesvm` plus `litesvm-utils` and covers:
- successful `create_escrow -> allow_mint -> deposit -> withdraw`
- withdrawal rejection for a non-depositor

## Test Implementation Notes

- The Anchor tests load `anchor/target/deploy/escrow.so` and use the generated Anchor account/instruction types directly.
- The Pinocchio tests load `pinocchio/target/deploy/escrow_pinocchio.so`, provision program-owned PDA accounts manually, and then drive the program with raw `Instruction` values.
- Host-side Rust tests for both crates are configured to accept `target_os = "solana"` in `check-cfg`, which removes the upstream macro warnings during `cargo test`.

## Notes

- The Anchor escrow example declares the internal `anchor-debug`, `custom-heap`, and `custom-panic` feature names expected by modern Rust `check-cfg`, so it should build cleanly without those macro warnings.
- `Anchor.toml` pins this example to Anchor CLI `1.0.0`. Using a different installed Anchor version may produce version mismatch warnings even if the build still succeeds.

## Sources

- Official escrow repository: https://github.com/solana-program/escrow
- Technical reference used for the Pinocchio-aligned flow:
  https://github.com/solana-program/escrow/blob/main/docs/PROGRAM_OVERVIEW.md
