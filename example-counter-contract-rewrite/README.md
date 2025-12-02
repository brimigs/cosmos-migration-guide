# Counter Contract: CosmWasm vs Anchor

Side-by-side comparison of the same counter logic implemented in CosmWasm (Cosmos) and Anchor (Solana).

## Key Differences

| Aspect | CosmWasm | Anchor (Solana) |
|--------|----------|-----------------|
| **Entry points** | Multiple (`instantiate`, `execute`, `query`) | Single (`#[program]` module with functions) |
| **State storage** | Key-value store (`Item`, `Map`) | Account data (PDAs) |
| **State location** | Contract owns its storage | Accounts passed in per transaction |
| **Message routing** | Enum matching in `execute()` | Separate functions with `Context` |
| **Initialization** | `instantiate` entry point | `initialize` instruction creates account |
| **Access control** | Check `info.sender` manually | `Signer<'info>` + PDA seeds |

## Code Comparison

### State Definition

**CosmWasm** - Storage items within contract:
```rust
const COUNTER: Item<i64> = Item::new("counter");
const OWNER: Item<String> = Item::new("owner");
```

**Anchor** - Account struct with explicit size:
```rust
#[account]
#[derive(InitSpace)]
pub struct Counter {
    pub count: i64,
    pub owner: Pubkey,
    pub bump: u8,
}
```

### Initialization

**CosmWasm** - Dedicated entry point:
```rust
#[entry_point]
pub fn instantiate(deps: DepsMut, _env: Env, info: MessageInfo, msg: InstantiateMsg) -> StdResult<Response> {
    COUNTER.save(deps.storage, &msg.initial_count)?;
    OWNER.save(deps.storage, &info.sender.to_string())?;
    Ok(Response::new())
}
```

**Anchor** - Instruction that creates PDA:
```rust
pub fn initialize(ctx: Context<Initialize>, initial_count: i64) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    counter.count = initial_count;
    counter.owner = ctx.accounts.owner.key();
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = owner, space = 8 + Counter::INIT_SPACE, seeds = [b"counter", owner.key().as_ref()], bump)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
```

### Increment

**CosmWasm** - Update storage item:
```rust
ExecuteMsg::Increment {} => {
    COUNTER.update(deps.storage, |count| -> StdResult<i64> { Ok(count + 1) })?;
    Ok(Response::new())
}
```

**Anchor** - Modify account data directly:
```rust
pub fn increment(ctx: Context<Update>) -> Result<()> {
    ctx.accounts.counter.count += 1;
    Ok(())
}
```

## Migration Considerations

1. **State becomes accounts**: Each piece of state needs a corresponding account struct
2. **PDAs replace storage keys**: Use `seeds` to derive deterministic addresses
3. **Account validation is explicit**: Anchor's `#[derive(Accounts)]` defines constraints
4. **No built-in queries**: Read state via RPC `getAccountInfo`, not on-chain queries
5. **Rent required**: Accounts must hold SOL for rent exemption

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
