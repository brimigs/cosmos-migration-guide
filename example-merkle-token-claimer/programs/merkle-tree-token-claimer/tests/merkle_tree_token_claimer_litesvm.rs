use anchor_lang::{prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_litesvm::{AnchorContext, AnchorLiteSVM};
use merkle_tree_token_claimer::{
    instruction::{ClaimAirdrop, InitializeAirdropData, UpdateTree},
    AirdropState, ClaimError, ClaimReceipt,
};
use std::fs;
use std::path::PathBuf;
use sha2::{Digest, Sha256};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_sdk_ids::system_program;
use solana_signer::Signer;
use spl_associated_token_account_client::program as associated_token_program;
use spl_token::{
    solana_program::program_pack::Pack,
    state::{Account as TokenAccount, Mint},
    ID as TOKEN_PROGRAM_ID,
};

const CLAIM_RECEIPT_ALREADY_PROCESSED: u32 = ClaimError::AlreadyClaimed as u32 + 6000;
const INVALID_PROOF: u32 = ClaimError::InvalidProof as u32 + 6000;
const CLAIMS_STARTED: u32 = ClaimError::ClaimsStarted as u32 + 6000;

#[derive(Debug)]
struct Claimant {
    wallet: Keypair,
    amount: u64,
}

struct TestEnv {
    ctx: AnchorContext,
    authority: Keypair,
    mint: Keypair,
    claimants: Vec<Claimant>,
    order: Vec<usize>,
    tree: TestMerkleTree,
    airdrop_state: Pubkey,
    vault: Pubkey,
    total_airdrop_amount: u64,
}

impl TestEnv {
    fn new() -> Self {
        let mut ctx = AnchorLiteSVM::build_with_program(
            merkle_tree_token_claimer::ID,
            &load_program_bytes(),
        );

        let authority = Keypair::new();
        ctx.svm
            .airdrop(&authority.pubkey(), 10 * LAMPORTS_PER_SOL)
            .unwrap();

        let claimants = vec![
            Claimant {
                wallet: Keypair::new(),
                amount: 125,
            },
            Claimant {
                wallet: Keypair::new(),
                amount: 275,
            },
            Claimant {
                wallet: Keypair::new(),
                amount: 400,
            },
        ];

        for claimant in &claimants {
            ctx.svm
                .airdrop(&claimant.wallet.pubkey(), LAMPORTS_PER_SOL)
                .unwrap();
        }

        let mint = Keypair::new();
        let order = (0..claimants.len()).collect::<Vec<_>>();
        let tree = Self::build_tree(&claimants, &order);
        let airdrop_state = Pubkey::find_program_address(
            &[b"merkle_tree", mint.pubkey().as_ref()],
            &merkle_tree_token_claimer::ID,
        )
        .0;
        let vault = associated_token_address(&airdrop_state, &mint.pubkey());
        let total_airdrop_amount = claimants.iter().map(|claimant| claimant.amount).sum();

        Self {
            ctx,
            authority,
            mint,
            claimants,
            order,
            tree,
            airdrop_state,
            vault,
            total_airdrop_amount,
        }
    }

    fn build_tree(claimants: &[Claimant], order: &[usize]) -> TestMerkleTree {
        let leaves = order
            .iter()
            .map(|&index| {
                let claimant = &claimants[index];
                Self::leaf_bytes(&claimant.wallet.pubkey(), claimant.amount)
            })
            .collect::<Vec<_>>();
        TestMerkleTree::new(leaves)
    }

    fn leaf_bytes(wallet: &Pubkey, amount: u64) -> Vec<u8> {
        let mut leaf = Vec::with_capacity(40);
        leaf.extend_from_slice(wallet.as_ref());
        leaf.extend_from_slice(&amount.to_le_bytes());
        leaf
    }

    fn merkle_root(&self) -> [u8; 32] {
        self.tree.root()
    }

    fn initialize_airdrop(&mut self) {
        let ix = Instruction {
            program_id: merkle_tree_token_claimer::ID,
            accounts: merkle_tree_token_claimer::accounts::Initialize {
                airdrop_state: self.airdrop_state,
                mint: self.mint.pubkey(),
                vault: self.vault,
                authority: self.authority.pubkey(),
                system_program: system_program::ID,
                token_program: TOKEN_PROGRAM_ID,
                associated_token_program: associated_token_program_address(),
            }
            .to_account_metas(None),
            data: InitializeAirdropData {
                merkle_root: self.merkle_root(),
                amount: self.total_airdrop_amount,
            }
            .data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.authority, &self.mint])
            .unwrap()
            .assert_success();
    }

    fn reverse_tree(&mut self) -> [u8; 32] {
        self.order.reverse();
        self.tree = Self::build_tree(&self.claimants, &self.order);
        self.merkle_root()
    }

    fn update_tree(&mut self, new_root: [u8; 32]) {
        let ix = Instruction {
            program_id: merkle_tree_token_claimer::ID,
            accounts: merkle_tree_token_claimer::accounts::Update {
                airdrop_state: self.airdrop_state,
                mint: self.mint.pubkey(),
                authority: self.authority.pubkey(),
            }
            .to_account_metas(None),
            data: UpdateTree { new_root }.data(),
        };

        self.ctx
            .execute_instruction(ix, &[&self.authority])
            .unwrap()
            .assert_success();
    }

    fn proof_for(&self, claimant_index: usize) -> (u64, Vec<u8>) {
        let position = self
            .order
            .iter()
            .position(|index| *index == claimant_index)
            .unwrap() as u64;
        let proof = self
            .tree
            .proof(position as usize);
        (position, proof)
    }

    fn claim_receipt(&self, index: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[
                b"claim_receipt",
                self.airdrop_state.as_ref(),
                &index.to_le_bytes(),
            ],
            &merkle_tree_token_claimer::ID,
        )
        .0
    }

    fn signer_ata(&self, wallet: &Pubkey) -> Pubkey {
        associated_token_address(wallet, &self.mint.pubkey())
    }

    fn claim_ix(
        &self,
        signer: &Pubkey,
        amount: u64,
        proof: Vec<u8>,
        index: u64,
    ) -> Instruction {
        let signer_ata = self.signer_ata(signer);
        let claim_receipt = self.claim_receipt(index);
        Instruction {
            program_id: merkle_tree_token_claimer::ID,
            accounts: merkle_tree_token_claimer::accounts::Claim {
                airdrop_state: self.airdrop_state,
                mint: self.mint.pubkey(),
                vault: self.vault,
                claim_receipt,
                signer_ata,
                signer: *signer,
                system_program: system_program::ID,
                token_program: TOKEN_PROGRAM_ID,
                associated_token_program: associated_token_program_address(),
            }
            .to_account_metas(None),
            data: ClaimAirdrop {
                amount,
                hashes: proof,
                index,
            }
            .data(),
        }
    }

    fn claim_success(&mut self, claimant_index: usize) -> u64 {
        let claimant = &self.claimants[claimant_index];
        let (index, proof) = self.proof_for(claimant_index);
        let ix = self.claim_ix(&claimant.wallet.pubkey(), claimant.amount, proof, index);
        self.ctx
            .execute_instruction(ix, &[&claimant.wallet])
            .unwrap()
            .assert_success();
        index
    }

    fn read_anchor_account<T: AccountDeserialize>(&self, address: &Pubkey) -> T {
        self.ctx.get_account(address).unwrap()
    }

    fn read_token_account(&self, address: &Pubkey) -> TokenAccount {
        let account = self.ctx.svm.get_account(address).unwrap();
        TokenAccount::unpack(&account.data).unwrap()
    }

    fn read_mint(&self) -> Mint {
        let account = self.ctx.svm.get_account(&self.mint.pubkey()).unwrap();
        Mint::unpack(&account.data).unwrap()
    }
}

#[test]
fn initializes_and_updates_before_claims() {
    let mut env = TestEnv::new();
    env.initialize_airdrop();

    let state: AirdropState = env.read_anchor_account(&env.airdrop_state);
    let mint = env.read_mint();
    let vault = env.read_token_account(&env.vault);

    assert_eq!(state.merkle_root, env.merkle_root());
    assert_eq!(state.airdrop_amount, env.total_airdrop_amount);
    assert_eq!(state.amount_claimed, 0);
    assert_eq!(state.authority, env.authority.pubkey());
    assert_eq!(mint.supply, env.total_airdrop_amount);
    assert!(mint.mint_authority.is_none());
    assert_eq!(vault.amount, env.total_airdrop_amount);

    let updated_root = env.reverse_tree();
    env.update_tree(updated_root);

    let updated_state: AirdropState = env.read_anchor_account(&env.airdrop_state);
    assert_eq!(updated_state.merkle_root, updated_root);
    assert_eq!(updated_state.amount_claimed, 0);
}

#[test]
fn records_receipts_and_keeps_other_proofs_valid() {
    let mut env = TestEnv::new();
    env.initialize_airdrop();
    env.reverse_tree();
    let updated_root = env.merkle_root();
    env.update_tree(updated_root);

    let first_index = env.claim_success(0);
    let first_receipt: ClaimReceipt = env.read_anchor_account(&env.claim_receipt(first_index));
    let first_signer_ata = env.signer_ata(&env.claimants[0].wallet.pubkey());
    let first_ata = env.read_token_account(&first_signer_ata);
    let state_after_first: AirdropState = env.read_anchor_account(&env.airdrop_state);

    assert_eq!(first_ata.amount, env.claimants[0].amount);
    assert_eq!(first_receipt.claimer, env.claimants[0].wallet.pubkey());
    assert_eq!(first_receipt.amount, env.claimants[0].amount);
    assert_eq!(state_after_first.amount_claimed, env.claimants[0].amount);

    let second_index = env.claim_success(1);
    let second_signer_ata = env.signer_ata(&env.claimants[1].wallet.pubkey());
    let second_ata = env.read_token_account(&second_signer_ata);
    let vault = env.read_token_account(&env.vault);
    let state_after_second: AirdropState = env.read_anchor_account(&env.airdrop_state);
    let second_receipt: ClaimReceipt = env.read_anchor_account(&env.claim_receipt(second_index));

    assert_eq!(second_ata.amount, env.claimants[1].amount);
    assert_eq!(
        state_after_second.amount_claimed,
        env.claimants[0].amount + env.claimants[1].amount
    );
    assert_eq!(
        vault.amount,
        env.total_airdrop_amount - env.claimants[0].amount - env.claimants[1].amount
    );
    assert_eq!(second_receipt.index, second_index);
    assert_eq!(second_receipt.amount, env.claimants[1].amount);
}

#[test]
fn rejects_duplicate_and_invalid_claims() {
    let mut env = TestEnv::new();
    env.initialize_airdrop();

    let claimed_index = env.claim_success(0);
    let (duplicate_index, duplicate_proof) = env.proof_for(0);
    assert_eq!(claimed_index, duplicate_index);

    let duplicate_ix = env.claim_ix(
        &env.claimants[0].wallet.pubkey(),
        env.claimants[0].amount,
        duplicate_proof,
        duplicate_index,
    );
    let duplicate_result = env
        .ctx
        .execute_instruction(duplicate_ix, &[&env.claimants[0].wallet])
        .unwrap();
    duplicate_result.assert_failure();
    duplicate_result.assert_error_code(CLAIM_RECEIPT_ALREADY_PROCESSED);

    let attacker = Keypair::new();
    env.ctx
        .svm
        .airdrop(&attacker.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();
    let (victim_index, victim_proof) = env.proof_for(2);
    let invalid_ix = env.claim_ix(
        &attacker.pubkey(),
        env.claimants[2].amount,
        victim_proof,
        victim_index,
    );
    let invalid_result = env
        .ctx
        .execute_instruction(invalid_ix, &[&attacker])
        .unwrap();
    invalid_result.assert_failure();
    invalid_result.assert_error_code(INVALID_PROOF);
}

#[test]
fn rejects_root_updates_after_claims_begin() {
    let mut env = TestEnv::new();
    env.initialize_airdrop();
    env.claim_success(0);

        let update_ix = Instruction {
        program_id: merkle_tree_token_claimer::ID,
        accounts: merkle_tree_token_claimer::accounts::Update {
            airdrop_state: env.airdrop_state,
            mint: env.mint.pubkey(),
            authority: env.authority.pubkey(),
        }
        .to_account_metas(None),
        data: UpdateTree {
            new_root: env.merkle_root(),
        }
        .data(),
    };

    let result = env
        .ctx
        .execute_instruction(update_ix, &[&env.authority])
        .unwrap();
    result.assert_failure();
    result.assert_error_code(CLAIMS_STARTED);
}

struct TestMerkleTree {
    levels: Vec<Vec<[u8; 32]>>,
}

impl TestMerkleTree {
    fn new(leaves: Vec<Vec<u8>>) -> Self {
        let mut levels = vec![leaves.into_iter().map(|leaf| sha256(&leaf)).collect::<Vec<_>>()];
        while levels.last().unwrap().len() > 1 {
            let current = levels.last().unwrap();
            let mut next = Vec::with_capacity(current.len().div_ceil(2));
            for pair in current.chunks(2) {
                let left = pair[0];
                let right = pair.get(1).copied().unwrap_or(left);
                next.push(hash_pair(&left, &right));
            }
            levels.push(next);
        }
        Self { levels }
    }

    fn root(&self) -> [u8; 32] {
        self.levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .unwrap()
    }

    fn proof(&self, mut index: usize) -> Vec<u8> {
        let mut proof = Vec::new();
        for level in &self.levels[..self.levels.len().saturating_sub(1)] {
            let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
            let sibling = level.get(sibling_index).copied().unwrap_or(level[index]);
            proof.extend_from_slice(&sibling);
            index /= 2;
        }
        proof
    }
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn load_program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/merkle_tree_token_claimer.so");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read program binary at {}: {error}",
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
