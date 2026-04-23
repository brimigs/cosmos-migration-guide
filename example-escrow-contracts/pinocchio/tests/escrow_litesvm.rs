use escrow_pinocchio::{AllowedMintAccount, EscrowAccount, ReceiptAccount};
use litesvm::LiteSVM;
use litesvm_utils::{AssertionHelpers, LiteSVMBuilder, TestHelpers, TransactionHelpers};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;
use solana_signer::Signer;
use spl_associated_token_account_client::program as associated_token_program;
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::{fs, path::PathBuf};

const CREATE_ESCROW_TAG: u8 = 0;
const DEPOSIT_TAG: u8 = 3;
const WITHDRAW_TAG: u8 = 5;
const ALLOW_MINT_TAG: u8 = 6;

const ESCROW_ACCOUNT_LEN: usize = 67;
const RECEIPT_ACCOUNT_LEN: usize = 154;
const ALLOWED_MINT_ACCOUNT_LEN: usize = 3;
const DEPOSIT_AMOUNT: u64 = 250_000;

struct TestEnv {
    svm: LiteSVM,
    program_id: Pubkey,
    payer: Keypair,
    admin: Keypair,
    depositor: Keypair,
    intruder: Keypair,
    mint: Keypair,
    escrow_seed: Keypair,
    receipt_seed: Keypair,
    escrow: Pubkey,
    escrow_bump: u8,
    allowed_mint: Pubkey,
    allowed_mint_bump: u8,
    receipt: Pubkey,
    receipt_bump: u8,
    vault: Pubkey,
    depositor_ata: Pubkey,
    intruder_ata: Pubkey,
}

impl TestEnv {
    fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let svm = LiteSVMBuilder::build_with_program(program_id, &load_program_bytes());
        let mut env = Self {
            svm,
            program_id,
            payer: Keypair::new(),
            admin: Keypair::new(),
            depositor: Keypair::new(),
            intruder: Keypair::new(),
            mint: Keypair::new(),
            escrow_seed: Keypair::new(),
            receipt_seed: Keypair::new(),
            escrow: Pubkey::default(),
            escrow_bump: 0,
            allowed_mint: Pubkey::default(),
            allowed_mint_bump: 0,
            receipt: Pubkey::default(),
            receipt_bump: 0,
            vault: Pubkey::default(),
            depositor_ata: Pubkey::default(),
            intruder_ata: Pubkey::default(),
        };

        env.fund_signers();
        env.mint = env.svm.create_token_mint(&env.admin, 6).unwrap();
        env.depositor_ata = env
            .svm
            .create_associated_token_account(&env.mint.pubkey(), &env.depositor)
            .unwrap();
        env.intruder_ata = env
            .svm
            .create_associated_token_account(&env.mint.pubkey(), &env.intruder)
            .unwrap();
        env.svm
            .mint_to(&env.mint.pubkey(), &env.depositor_ata, &env.admin, DEPOSIT_AMOUNT)
            .unwrap();

        (env.escrow, env.escrow_bump) = env
            .svm
            .get_pda_with_bump(&[b"escrow", env.escrow_seed.pubkey().as_ref()], &env.program_id);
        (env.allowed_mint, env.allowed_mint_bump) = env.svm.get_pda_with_bump(
            &[b"allowed_mint", env.escrow.as_ref(), env.mint.pubkey().as_ref()],
            &env.program_id,
        );
        (env.receipt, env.receipt_bump) = env.svm.get_pda_with_bump(
            &[
                b"receipt",
                env.escrow.as_ref(),
                env.depositor.pubkey().as_ref(),
                env.mint.pubkey().as_ref(),
                env.receipt_seed.pubkey().as_ref(),
            ],
            &env.program_id,
        );
        env.vault = associated_token_address(&env.escrow, &env.mint.pubkey());

        env
    }

    fn fund_signers(&mut self) {
        for key in [
            &self.payer,
            &self.admin,
            &self.depositor,
            &self.intruder,
            &self.escrow_seed,
            &self.receipt_seed,
        ] {
            self.svm.airdrop(&key.pubkey(), LAMPORTS_PER_SOL).unwrap();
        }
    }

    fn provision_program_account(&mut self, address: Pubkey, space: usize) {
        let lamports = self.svm.minimum_balance_for_rent_exemption(space);
        self.svm
            .set_account(
                address,
                Account {
                    lamports,
                    data: vec![0; space],
                    owner: self.program_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
    }

    fn initialize(&mut self) {
        self.provision_program_account(self.escrow, ESCROW_ACCOUNT_LEN);
        self.create_escrow().assert_success();

        self.provision_program_account(self.allowed_mint, ALLOWED_MINT_ACCOUNT_LEN);
        self.allow_mint().assert_success();
    }

    fn create_escrow(&mut self) -> litesvm_utils::TransactionResult {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.admin.pubkey(), true),
                AccountMeta::new_readonly(self.escrow_seed.pubkey(), true),
                AccountMeta::new(self.escrow, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: vec![CREATE_ESCROW_TAG, self.escrow_bump],
        };

        self.svm
            .send_instruction(ix, &[&self.payer, &self.admin, &self.escrow_seed])
            .unwrap()
    }

    fn allow_mint(&mut self) -> litesvm_utils::TransactionResult {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.admin.pubkey(), true),
                AccountMeta::new_readonly(self.escrow, false),
                AccountMeta::new_readonly(self.mint.pubkey(), false),
                AccountMeta::new(self.allowed_mint, false),
                AccountMeta::new(self.vault, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: vec![ALLOW_MINT_TAG, self.allowed_mint_bump],
        };

        self.svm
            .send_instruction(ix, &[&self.payer, &self.admin])
            .unwrap()
    }

    fn deposit(&mut self, amount: u64) -> litesvm_utils::TransactionResult {
        self.provision_program_account(self.receipt, RECEIPT_ACCOUNT_LEN);

        let mut data = Vec::with_capacity(10);
        data.push(DEPOSIT_TAG);
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(self.receipt_bump);

        let ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.depositor.pubkey(), true),
                AccountMeta::new_readonly(self.escrow, false),
                AccountMeta::new_readonly(self.allowed_mint, false),
                AccountMeta::new_readonly(self.receipt_seed.pubkey(), true),
                AccountMeta::new(self.receipt, false),
                AccountMeta::new(self.vault, false),
                AccountMeta::new(self.depositor_ata, false),
                AccountMeta::new_readonly(self.mint.pubkey(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        };

        self.svm
            .send_instruction(ix, &[&self.payer, &self.depositor, &self.receipt_seed])
            .unwrap()
    }

    fn withdraw_as_depositor(&mut self, destination: Pubkey) -> litesvm_utils::TransactionResult {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(self.admin.pubkey(), false),
                AccountMeta::new_readonly(self.depositor.pubkey(), true),
                AccountMeta::new_readonly(self.escrow, false),
                AccountMeta::new(self.receipt, false),
                AccountMeta::new(self.vault, false),
                AccountMeta::new(destination, false),
                AccountMeta::new_readonly(self.mint.pubkey(), false),
            ],
            data: vec![WITHDRAW_TAG],
        };

        self.svm
            .send_instruction(ix, &[&self.payer, &self.depositor])
            .unwrap()
    }

    fn withdraw_as_intruder(&mut self, destination: Pubkey) -> litesvm_utils::TransactionResult {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(self.admin.pubkey(), false),
                AccountMeta::new_readonly(self.intruder.pubkey(), true),
                AccountMeta::new_readonly(self.escrow, false),
                AccountMeta::new(self.receipt, false),
                AccountMeta::new(self.vault, false),
                AccountMeta::new(destination, false),
                AccountMeta::new_readonly(self.mint.pubkey(), false),
            ],
            data: vec![WITHDRAW_TAG],
        };

        self.svm
            .send_instruction(ix, &[&self.payer, &self.intruder])
            .unwrap()
    }
}

#[test]
fn pinocchio_escrow_deposit_and_withdraw_round_trip() {
    let mut env = TestEnv::new();
    env.initialize();

    env.deposit(DEPOSIT_AMOUNT).assert_success();

    let escrow = read_escrow(env.svm.get_account(&env.escrow).unwrap().data.as_slice());
    let allowed_mint = read_allowed_mint(
        env.svm
            .get_account(&env.allowed_mint)
            .unwrap()
            .data
            .as_slice(),
    );
    let receipt = read_receipt(env.svm.get_account(&env.receipt).unwrap().data.as_slice());

    assert_eq!(escrow.version, 1);
    assert_eq!(escrow.bump, env.escrow_bump);
    assert_eq!(escrow.escrow_seed, env.escrow_seed.pubkey().to_bytes());
    assert_eq!(escrow.admin, env.admin.pubkey().to_bytes());
    assert_eq!(allowed_mint.discriminator, ALLOW_MINT_TAG);
    assert_eq!(allowed_mint.bump, env.allowed_mint_bump);
    assert_eq!(receipt.escrow, env.escrow.to_bytes());
    assert_eq!(receipt.depositor, env.depositor.pubkey().to_bytes());
    assert_eq!(receipt.mint, env.mint.pubkey().to_bytes());
    assert_eq!(receipt.receipt_seed, env.receipt_seed.pubkey().to_bytes());
    assert_eq!(receipt.amount, DEPOSIT_AMOUNT);

    env.svm.assert_token_balance(&env.depositor_ata, 0);
    env.svm.assert_token_balance(&env.vault, DEPOSIT_AMOUNT);

    env.withdraw_as_depositor(env.depositor_ata).assert_success();

    env.svm.assert_token_balance(&env.depositor_ata, DEPOSIT_AMOUNT);
    env.svm.assert_token_balance(&env.vault, 0);

    let receipt_account = env.svm.get_account(&env.receipt).unwrap();
    assert!(receipt_account.data.iter().all(|byte| *byte == 0));
}

#[test]
fn pinocchio_escrow_rejects_wrong_withdrawer() {
    let mut env = TestEnv::new();
    env.initialize();
    env.deposit(DEPOSIT_AMOUNT).assert_success();

    let result = env.withdraw_as_intruder(env.intruder_ata);
    result.assert_failure().assert_error("InvalidAccountData");

    env.svm.assert_token_balance(&env.vault, DEPOSIT_AMOUNT);
    env.svm.assert_token_balance(&env.intruder_ata, 0);
    env.svm.assert_token_balance(&env.depositor_ata, 0);
}

fn read_escrow(data: &[u8]) -> EscrowAccount {
    EscrowAccount {
        discriminator: data[0],
        version: data[1],
        bump: data[2],
        escrow_seed: data[3..35].try_into().unwrap(),
        admin: data[35..67].try_into().unwrap(),
    }
}

fn read_allowed_mint(data: &[u8]) -> AllowedMintAccount {
    AllowedMintAccount {
        discriminator: data[0],
        version: data[1],
        bump: data[2],
    }
}

fn read_receipt(data: &[u8]) -> ReceiptAccount {
    ReceiptAccount {
        discriminator: data[0],
        version: data[1],
        bump: data[2],
        padding: data[3..10].try_into().unwrap(),
        escrow: data[10..42].try_into().unwrap(),
        depositor: data[42..74].try_into().unwrap(),
        mint: data[74..106].try_into().unwrap(),
        receipt_seed: data[106..138].try_into().unwrap(),
        amount: u64::from_le_bytes(data[138..146].try_into().unwrap()),
        deposited_at: i64::from_le_bytes(data[146..154].try_into().unwrap()),
    }
}

fn load_program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/deploy/escrow_pinocchio.so");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read Pinocchio escrow program binary at {}: {error}. Run `cargo build-sbf --features bpf-entrypoint` in example-escrow-contracts/pinocchio first.",
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
