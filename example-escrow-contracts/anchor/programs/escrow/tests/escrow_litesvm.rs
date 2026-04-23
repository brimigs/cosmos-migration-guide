use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM};
use escrow::{instruction, AllowedMint, Escrow, EscrowError, Receipt, ID};
use litesvm_utils::{AssertionHelpers, TestHelpers};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_sdk_ids::system_program;
use solana_signer::Signer;
use spl_associated_token_account_client::program as associated_token_program;
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::{fs, path::PathBuf};

const ZERO_DEPOSIT_AMOUNT: u32 = EscrowError::ZeroDepositAmount as u32 + 6000;
const DEPOSIT_AMOUNT: u64 = 250_000;

struct TestEnv {
    ctx: AnchorContext,
    admin: Keypair,
    depositor: Keypair,
    mint: Keypair,
    escrow_seed: Pubkey,
    receipt_seed: Pubkey,
    escrow: Pubkey,
    allowed_mint: Pubkey,
    vault: Pubkey,
    depositor_ata: Pubkey,
    receipt: Pubkey,
}

impl TestEnv {
    fn new() -> Self {
        let mut ctx = AnchorLiteSVM::build_with_program(ID, &load_program_bytes());

        let admin = ctx
            .svm
            .create_funded_account(10 * LAMPORTS_PER_SOL)
            .unwrap();
        let depositor = ctx
            .svm
            .create_funded_account(10 * LAMPORTS_PER_SOL)
            .unwrap();
        let mint = ctx.svm.create_token_mint(&admin, 6).unwrap();
        let depositor_ata = ctx
            .svm
            .create_associated_token_account(&mint.pubkey(), &depositor)
            .unwrap();
        ctx.svm
            .mint_to(&mint.pubkey(), &depositor_ata, &admin, DEPOSIT_AMOUNT)
            .unwrap();

        let escrow_seed = Pubkey::new_unique();
        let receipt_seed = Pubkey::new_unique();
        let escrow = ctx
            .svm
            .get_pda(&[b"escrow", escrow_seed.as_ref()], &ID);
        let allowed_mint = ctx
            .svm
            .get_pda(&[b"allowed_mint", escrow.as_ref(), mint.pubkey().as_ref()], &ID);
        let receipt = ctx.svm.get_pda(
            &[
                b"receipt",
                escrow.as_ref(),
                depositor.pubkey().as_ref(),
                mint.pubkey().as_ref(),
                receipt_seed.as_ref(),
            ],
            &ID,
        );
        let vault = associated_token_address(&escrow, &mint.pubkey());

        Self {
            ctx,
            admin,
            depositor,
            mint,
            escrow_seed,
            receipt_seed,
            escrow,
            allowed_mint,
            vault,
            depositor_ata,
            receipt,
        }
    }

    fn initialize(&mut self) {
        self.create_escrow();
        self.allow_mint();
    }

    fn create_escrow(&mut self) {
        let ix = Instruction {
            program_id: ID,
            accounts: escrow::accounts::CreateEscrow {
                escrow: self.escrow,
                payer: self.admin.pubkey(),
                admin: self.admin.pubkey(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: instruction::CreateEscrow {
                escrow_seed: self.escrow_seed,
            }
            .data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.admin])
            .unwrap()
            .assert_success();
    }

    fn allow_mint(&mut self) {
        let ix = Instruction {
            program_id: ID,
            accounts: escrow::accounts::AllowMint {
                escrow: self.escrow,
                admin: self.admin.pubkey(),
                mint: self.mint.pubkey(),
                allowed_mint: self.allowed_mint,
                vault: self.vault,
                payer: self.admin.pubkey(),
                token_program: TOKEN_PROGRAM_ID,
                associated_token_program: associated_token_program_address(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: instruction::AllowMint {}.data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.admin])
            .unwrap()
            .assert_success();
    }

    fn deposit(&mut self, amount: u64) -> anchor_litesvm::TransactionResult {
        let ix = Instruction {
            program_id: ID,
            accounts: escrow::accounts::Deposit {
                escrow: self.escrow,
                allowed_mint: self.allowed_mint,
                receipt: self.receipt,
                vault: self.vault,
                depositor_token_account: self.depositor_ata,
                mint: self.mint.pubkey(),
                payer: self.depositor.pubkey(),
                depositor: self.depositor.pubkey(),
                token_program: TOKEN_PROGRAM_ID,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: instruction::Deposit {
                receipt_seed: self.receipt_seed,
                amount,
            }
            .data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.depositor])
            .unwrap()
    }

    fn withdraw(&mut self) -> anchor_litesvm::TransactionResult {
        let ix = Instruction {
            program_id: ID,
            accounts: escrow::accounts::Withdraw {
                escrow: self.escrow,
                receipt: self.receipt,
                vault: self.vault,
                withdrawer_token_account: self.depositor_ata,
                mint: self.mint.pubkey(),
                payer: self.depositor.pubkey(),
                rent_recipient: self.admin.pubkey(),
                withdrawer: self.depositor.pubkey(),
                depositor: self.depositor.pubkey(),
                token_program: TOKEN_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: instruction::Withdraw {}.data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.depositor])
            .unwrap()
    }
}

#[test]
fn anchor_escrow_deposit_and_withdraw_round_trip() {
    let mut env = TestEnv::new();
    env.initialize();

    env.deposit(DEPOSIT_AMOUNT).assert_success();

    let escrow_state: Escrow = env.ctx.get_account(&env.escrow).unwrap();
    let allowed_mint: AllowedMint = env.ctx.get_account(&env.allowed_mint).unwrap();
    let receipt: Receipt = env.ctx.get_account(&env.receipt).unwrap();

    assert_eq!(escrow_state.admin, env.admin.pubkey());
    assert_eq!(escrow_state.escrow_seed, env.escrow_seed);
    assert_eq!(allowed_mint.version, 1);
    assert_eq!(receipt.escrow, env.escrow);
    assert_eq!(receipt.depositor, env.depositor.pubkey());
    assert_eq!(receipt.mint, env.mint.pubkey());
    assert_eq!(receipt.receipt_seed, env.receipt_seed);
    assert_eq!(receipt.amount, DEPOSIT_AMOUNT);

    env.ctx.svm.assert_token_balance(&env.depositor_ata, 0);
    env.ctx.svm.assert_token_balance(&env.vault, DEPOSIT_AMOUNT);

    env.withdraw().assert_success();

    env.ctx.svm.assert_token_balance(&env.depositor_ata, DEPOSIT_AMOUNT);
    env.ctx.svm.assert_token_balance(&env.vault, 0);
    env.ctx.svm.assert_account_closed(&env.receipt);
}

#[test]
fn anchor_escrow_rejects_zero_amount_deposit() {
    let mut env = TestEnv::new();
    env.initialize();

    let result = env.deposit(0);
    result
        .assert_failure()
        .assert_error(&format!("Custom({ZERO_DEPOSIT_AMOUNT})"))
        .assert_anchor_error("ZeroDepositAmount");

    env.ctx.svm.assert_account_closed(&env.receipt);
    env.ctx.svm.assert_token_balance(&env.depositor_ata, DEPOSIT_AMOUNT);
    env.ctx.svm.assert_token_balance(&env.vault, 0);
}

fn load_program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/escrow.so");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read Anchor escrow program binary at {}: {error}",
            path.display()
        )
    })
}

fn associated_token_address(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &associated_token_program_address(),
    )
    .0
}

fn associated_token_program_address() -> Pubkey {
    associated_token_program::ID.to_bytes().into()
}
