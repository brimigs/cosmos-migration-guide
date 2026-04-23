#![no_std]

use core::convert::TryInto;

use pinocchio::{
    instruction::cpi::{Seed, Signer},
    no_allocator, nostd_panic_handler, program_entrypoint, AccountView, Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::instructions::Transfer;

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

// Adapted to the architecture documented in:
// https://github.com/solana-program/escrow
//
// The official Solana escrow program is a receipt-based deposit/withdraw system,
// not the smaller arbiter-release escrow used in the Anchor/CosmWasm example.

const CREATE_ESCROW_TAG: u8 = 0;
const DEPOSIT_TAG: u8 = 3;
const WITHDRAW_TAG: u8 = 5;
const ALLOW_MINT_TAG: u8 = 6;

const ESCROW_ACCOUNT_LEN: usize = 67;
const RECEIPT_ACCOUNT_LEN: usize = 154;
const ALLOWED_MINT_ACCOUNT_LEN: usize = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EscrowAccount {
    pub discriminator: u8,
    pub version: u8,
    pub bump: u8,
    pub escrow_seed: [u8; 32],
    pub admin: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AllowedMintAccount {
    pub discriminator: u8,
    pub version: u8,
    pub bump: u8,
}

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (tag, rest) = instruction_data
        .split_first()
        .ok_or(pinocchio::error::ProgramError::InvalidInstructionData)?;

    match *tag {
        CREATE_ESCROW_TAG => create_escrow(program_id, accounts, rest),
        ALLOW_MINT_TAG => allow_mint(program_id, accounts, rest),
        DEPOSIT_TAG => deposit(program_id, accounts, rest),
        WITHDRAW_TAG => withdraw(program_id, accounts),
        _ => Err(pinocchio::error::ProgramError::InvalidInstructionData),
    }
}

fn create_escrow(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let bump = *instruction_data
        .first()
        .ok_or(pinocchio::error::ProgramError::InvalidInstructionData)?;

    let [payer, admin, escrow_seed, escrow, system_program] = accounts else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };

    require_signer(payer)?;
    require_signer(admin)?;
    require_signer(escrow_seed)?;
    require_writable(escrow)?;
    require_owned_by_program(escrow, program_id)?;
    require_data_len(escrow, ESCROW_ACCOUNT_LEN)?;

    let state = EscrowAccount {
        discriminator: CREATE_ESCROW_TAG,
        version: 1,
        bump,
        escrow_seed: address_bytes(escrow_seed.address()),
        admin: address_bytes(admin.address()),
    };
    state.store(escrow)?;

    let _ = system_program;
    Ok(())
}

fn allow_mint(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let bump = *instruction_data
        .first()
        .ok_or(pinocchio::error::ProgramError::InvalidInstructionData)?;

    let [payer, admin, escrow, mint, allowed_mint, vault, token_program, system_program] = accounts
    else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };

    require_signer(payer)?;
    require_signer(admin)?;
    require_writable(allowed_mint)?;
    require_writable(vault)?;
    require_owned_by_program(escrow, program_id)?;
    require_owned_by_program(allowed_mint, program_id)?;
    require_admin(escrow, admin)?;
    require_data_len(allowed_mint, ALLOWED_MINT_ACCOUNT_LEN)?;

    CreateIdempotent {
        funding_account: payer,
        account: vault,
        wallet: escrow,
        mint,
        system_program,
        token_program,
    }
    .invoke()?;

    AllowedMintAccount {
        discriminator: ALLOW_MINT_TAG,
        version: 1,
        bump,
    }
    .store(allowed_mint)?;

    Ok(())
}

fn deposit(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let amount = u64::from_le_bytes(
        instruction_data
            .get(..8)
            .ok_or(pinocchio::error::ProgramError::InvalidInstructionData)?
            .try_into()
            .map_err(|_| pinocchio::error::ProgramError::InvalidInstructionData)?,
    );
    let bump = *instruction_data
        .get(8)
        .ok_or(pinocchio::error::ProgramError::InvalidInstructionData)?;

    let [
        payer,
        depositor,
        escrow,
        allowed_mint,
        receipt_seed,
        receipt,
        vault,
        depositor_token_account,
        mint,
        token_program,
        system_program,
    ] = accounts
    else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };

    require_signer(payer)?;
    require_signer(depositor)?;
    require_signer(receipt_seed)?;
    require_writable(receipt)?;
    require_writable(vault)?;
    require_writable(depositor_token_account)?;
    require_owned_by_program(escrow, program_id)?;
    require_owned_by_program(allowed_mint, program_id)?;
    require_owned_by_program(receipt, program_id)?;
    require_data_len(receipt, RECEIPT_ACCOUNT_LEN)?;

    Transfer::new(depositor_token_account, vault, depositor, amount).invoke()?;

    // The official program stores the deposit timestamp and later enforces optional
    // timelock / hook extensions during withdrawal. This compact example mirrors the
    // receipt layout without reproducing the full extension machinery.
    ReceiptAccount {
        discriminator: DEPOSIT_TAG,
        version: 1,
        bump,
        padding: [0; 7],
        escrow: address_bytes(escrow.address()),
        depositor: address_bytes(depositor.address()),
        mint: address_bytes(mint.address()),
        receipt_seed: address_bytes(receipt_seed.address()),
        amount,
        deposited_at: 0,
    }
    .store(receipt)?;

    let _ = token_program;
    let _ = system_program;
    Ok(())
}

fn withdraw(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [payer, rent_recipient, withdrawer, escrow, receipt, vault, withdrawer_token_account, mint] =
        accounts
    else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };

    require_signer(payer)?;
    require_signer(withdrawer)?;
    require_writable(receipt)?;
    require_writable(vault)?;
    require_writable(withdrawer_token_account)?;
    require_owned_by_program(escrow, program_id)?;
    require_owned_by_program(receipt, program_id)?;

    let escrow_state = EscrowAccount::load(escrow)?;
    let receipt_state = ReceiptAccount::load(receipt)?;

    if receipt_state.escrow != address_bytes(escrow.address()) {
        return Err(pinocchio::error::ProgramError::InvalidAccountData);
    }
    if receipt_state.depositor != address_bytes(withdrawer.address()) {
        return Err(pinocchio::error::ProgramError::InvalidAccountData);
    }
    if receipt_state.mint != address_bytes(mint.address()) {
        return Err(pinocchio::error::ProgramError::InvalidAccountData);
    }

    let bump_ref = [escrow_state.bump];
    let signer_seeds = [
        Seed::from(b"escrow".as_slice()),
        Seed::from(escrow_state.escrow_seed.as_slice()),
        Seed::from(bump_ref.as_slice()),
    ];
    let signer = Signer::from(&signer_seeds[..]);

    Transfer::new(vault, withdrawer_token_account, escrow, receipt_state.amount)
        .invoke_signed(&[signer])?;

    receipt.try_borrow_mut()?.fill(0);
    let _ = rent_recipient;
    Ok(())
}

impl EscrowAccount {
    fn load(account: &AccountView) -> Result<Self, pinocchio::error::ProgramError> {
        let data = account.try_borrow()?;
        if data.len() < ESCROW_ACCOUNT_LEN {
            return Err(pinocchio::error::ProgramError::InvalidAccountData);
        }

        Ok(Self {
            discriminator: data[0],
            version: data[1],
            bump: data[2],
            escrow_seed: data[3..35]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            admin: data[35..67]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
        })
    }

    fn store(&self, account: &mut AccountView) -> Result<(), pinocchio::error::ProgramError> {
        let data = &mut account.try_borrow_mut()?;
        if data.len() < ESCROW_ACCOUNT_LEN {
            return Err(pinocchio::error::ProgramError::InvalidAccountData);
        }

        data[0] = self.discriminator;
        data[1] = self.version;
        data[2] = self.bump;
        data[3..35].copy_from_slice(&self.escrow_seed);
        data[35..67].copy_from_slice(&self.admin);
        Ok(())
    }
}

impl ReceiptAccount {
    fn load(account: &AccountView) -> Result<Self, pinocchio::error::ProgramError> {
        let data = account.try_borrow()?;
        if data.len() < RECEIPT_ACCOUNT_LEN {
            return Err(pinocchio::error::ProgramError::InvalidAccountData);
        }

        Ok(Self {
            discriminator: data[0],
            version: data[1],
            bump: data[2],
            padding: data[3..10]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            escrow: data[10..42]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            depositor: data[42..74]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            mint: data[74..106]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            receipt_seed: data[106..138]
                .try_into()
                .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            amount: u64::from_le_bytes(
                data[138..146]
                    .try_into()
                    .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            ),
            deposited_at: i64::from_le_bytes(
                data[146..154]
                    .try_into()
                    .map_err(|_| pinocchio::error::ProgramError::InvalidAccountData)?,
            ),
        })
    }

    fn store(&self, account: &mut AccountView) -> Result<(), pinocchio::error::ProgramError> {
        let data = &mut account.try_borrow_mut()?;
        if data.len() < RECEIPT_ACCOUNT_LEN {
            return Err(pinocchio::error::ProgramError::InvalidAccountData);
        }

        data[0] = self.discriminator;
        data[1] = self.version;
        data[2] = self.bump;
        data[3..10].copy_from_slice(&self.padding);
        data[10..42].copy_from_slice(&self.escrow);
        data[42..74].copy_from_slice(&self.depositor);
        data[74..106].copy_from_slice(&self.mint);
        data[106..138].copy_from_slice(&self.receipt_seed);
        data[138..146].copy_from_slice(&self.amount.to_le_bytes());
        data[146..154].copy_from_slice(&self.deposited_at.to_le_bytes());
        Ok(())
    }
}

impl AllowedMintAccount {
    fn store(&self, account: &mut AccountView) -> Result<(), pinocchio::error::ProgramError> {
        let data = &mut account.try_borrow_mut()?;
        if data.len() < ALLOWED_MINT_ACCOUNT_LEN {
            return Err(pinocchio::error::ProgramError::InvalidAccountData);
        }

        data[0] = self.discriminator;
        data[1] = self.version;
        data[2] = self.bump;
        Ok(())
    }
}

fn require_signer(account: &AccountView) -> ProgramResult {
    if account.is_signer() {
        Ok(())
    } else {
        Err(pinocchio::error::ProgramError::MissingRequiredSignature)
    }
}

fn require_writable(account: &AccountView) -> ProgramResult {
    if account.is_writable() {
        Ok(())
    } else {
        Err(pinocchio::error::ProgramError::InvalidAccountData)
    }
}

fn require_owned_by_program(account: &AccountView, program_id: &Address) -> ProgramResult {
    if account.owner() == program_id {
        Ok(())
    } else {
        Err(pinocchio::error::ProgramError::IncorrectProgramId)
    }
}

fn require_data_len(account: &AccountView, len: usize) -> ProgramResult {
    if account.data_len() >= len {
        Ok(())
    } else {
        Err(pinocchio::error::ProgramError::InvalidAccountData)
    }
}

fn require_admin(escrow: &AccountView, admin: &AccountView) -> ProgramResult {
    let escrow_state = EscrowAccount::load(escrow)?;
    if escrow_state.admin == address_bytes(admin.address()) {
        Ok(())
    } else {
        Err(pinocchio::error::ProgramError::InvalidAccountData)
    }
}

fn address_bytes(address: &Address) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(address.as_ref());
    bytes
}
