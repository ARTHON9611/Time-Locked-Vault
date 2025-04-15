use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
    program::{invoke, invoke_signed},
};
use spl_token::state::Account as TokenAccount;
use std::convert::TryFrom;

// Program entrypoint
entrypoint!(process_instruction);

// Program ID
solana_program::declare_id!("TimeLockedVault");

// Error codes
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Unlock time has not been reached")]
    UnlockTimeNotReached,
    
    #[error("Only the depositor can withdraw funds")]
    UnauthorizedWithdrawal,
    
    #[error("Deposit not found")]
    DepositNotFound,
    
    #[error("Invalid deposit amount")]
    InvalidAmount,
    
    #[error("Deposit already withdrawn")]
    AlreadyWithdrawn,
    
    #[error("Invalid unlock time")]
    InvalidUnlockTime,
    
    #[error("Reentrancy detected")]
    ReentrancyDetected,
    
    #[error("Invalid instruction data")]
    InvalidInstructionData,
    
    #[error("Account already in use")]
    AccountAlreadyInUse,
    
    #[error("Insufficient funds")]
    InsufficientFunds,
    
    #[error("Math overflow")]
    MathOverflow,
}

impl From<VaultError> for ProgramError {
    fn from(e: VaultError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

// Instruction types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum VaultInstruction {
    /// Create a new vault
    /// 
    /// Accounts expected:
    /// 0. `[signer]` The vault creator/owner
    /// 1. `[writable]` The vault account to be created
    /// 2. `[]` System program
    CreateVault,
    
    /// Deposit tokens into the vault
    /// 
    /// Accounts expected:
    /// 0. `[signer]` The depositor
    /// 1. `[writable]` The vault account
    /// 2. `[writable]` The token account to transfer from (owned by depositor)
    /// 3. `[writable]` The token account to transfer to (vault's token account)
    /// 4. `[]` The token program
    /// 5. `[]` The system program
    /// 6. `[]` The clock sysvar
    Deposit {
        /// Amount of tokens to deposit
        amount: u64,
        /// Timestamp when the deposit can be withdrawn
        unlock_time: i64,
        /// Optional tag for the deposit (e.g., "Vacation", "Rent")
        tag: [u8; 32],
    },
    
    /// Withdraw tokens from the vault
    /// 
    /// Accounts expected:
    /// 0. `[signer]` The depositor/owner
    /// 1. `[writable]` The vault account
    /// 2. `[writable]` The token account to transfer to (owned by depositor)
    /// 3. `[writable]` The token account to transfer from (vault's token account)
    /// 4. `[]` The token program
    /// 5. `[]` The clock sysvar
    Withdraw {
        /// Unique identifier for the deposit
        deposit_id: u64,
    },
    
    /// Emergency withdraw (requires multisig approval)
    /// 
    /// Accounts expected:
    /// 0. `[signer]` The emergency authority (multisig or DAO)
    /// 1. `[writable]` The vault account
    /// 2. `[writable]` The token account to transfer to (owned by depositor)
    /// 3. `[writable]` The token account to transfer from (vault's token account)
    /// 4. `[]` The token program
    /// 5. `[]` The depositor account
    EmergencyWithdraw {
        /// Unique identifier for the deposit
        deposit_id: u64,
    },
}

// Vault account data structure
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Vault {
    /// The owner of the vault
    pub owner: Pubkey,
    /// The number of deposits made to this vault
    pub deposit_count: u64,
    /// The deposits in this vault
    pub deposits: Vec<Deposit>,
    /// Reentrancy guard
    pub reentrancy_guard: bool,
    /// Emergency authority (multisig or DAO)
    pub emergency_authority: Option<Pubkey>,
}

// Deposit data structure
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Deposit {
    /// Unique identifier for the deposit
    pub id: u64,
    /// The depositor's public key
    pub depositor: Pubkey,
    /// The token mint address
    pub token_mint: Pubkey,
    /// Amount of tokens deposited
    pub amount: u64,
    /// Timestamp when the deposit can be withdrawn
    pub unlock_time: i64,
    /// Whether the deposit has been withdrawn
    pub withdrawn: bool,
    /// Optional tag for the deposit
    pub tag: [u8; 32],
    /// Creation timestamp
    pub created_at: i64,
}

// Process program instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Verify the instruction data is valid
    if instruction_data.is_empty() {
        return Err(VaultError::InvalidInstructionData.into());
    }
    
    let instruction = VaultInstruction::try_from_slice(instruction_data)
        .map_err(|_| VaultError::InvalidInstructionData)?;
    
    match instruction {
        VaultInstruction::CreateVault => process_create_vault(program_id, accounts),
        VaultInstruction::Deposit { amount, unlock_time, tag } => {
            process_deposit(program_id, accounts, amount, unlock_time, tag)
        },
        VaultInstruction::Withdraw { deposit_id } => {
            process_withdraw(program_id, accounts, deposit_id)
        },
        VaultInstruction::EmergencyWithdraw { deposit_id } => {
            process_emergency_withdraw(program_id, accounts, deposit_id)
        },
    }
}

// Process create vault instruction
fn process_create_vault(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let owner_info = next_account_info(account_info_iter)?;
    let vault_account_info = next_account_info(account_info_iter)?;
    
    // Verify the owner signed the transaction
    if !owner_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify the vault account is owned by the program
    if vault_account_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Check if the vault account is already initialized
    if !vault_account_info.data.borrow().is_empty() {
        return Err(VaultError::AccountAlreadyInUse.into());
    }
    
    // Initialize the vault
    let vault = Vault {
        owner: *owner_info.key,
        deposit_count: 0,
        deposits: Vec::new(),
        reentrancy_guard: false,
        emergency_authority: None,
    };
    
    // Serialize and store the vault data
    vault.serialize(&mut *vault_account_info.data.borrow_mut())?;
    
    msg!("Vault created successfully");
    Ok(())
}

// Process deposit instruction
fn process_deposit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    unlock_time: i64,
    tag: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let depositor_info = next_account_info(account_info_iter)?;
    let vault_account_info = next_account_info(account_info_iter)?;
    let source_token_account_info = next_account_info(account_info_iter)?;
    let destination_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;
    
    // Verify the depositor signed the transaction
    if !depositor_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify the vault account is owned by the program
    if vault_account_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Load the vault
    let mut vault = Vault::try_from_slice(&vault_account_info.data.borrow())?;
    
    // Check reentrancy guard
    if vault.reentrancy_guard {
        return Err(VaultError::ReentrancyDetected.into());
    }
    
    // Set reentrancy guard
    vault.reentrancy_guard = true;
    
    // Verify the amount is valid
    if amount == 0 {
        return Err(VaultError::InvalidAmount.into());
    }
    
    // Verify the unlock time is in the future
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    if unlock_time <= clock.unix_timestamp {
        return Err(VaultError::InvalidUnlockTime.into());
    }
    
    // Verify the source token account has sufficient funds
    let source_token_account = TokenAccount::unpack(&source_token_account_info.data.borrow())?;
    if source_token_account.amount < amount {
        return Err(VaultError::InsufficientFunds.into());
    }
    
    // Create a new deposit
    let deposit = Deposit {
        id: vault.deposit_count,
        depositor: *depositor_info.key,
        token_mint: source_token_account.mint,
        amount,
        unlock_time,
        withdrawn: false,
        tag,
        created_at: clock.unix_timestamp,
    };
    
    // Add the deposit to the vault
    vault.deposits.push(deposit);
    vault.deposit_count = vault.deposit_count.checked_add(1)
        .ok_or(VaultError::MathOverflow)?;
    
    // Transfer tokens from the depositor to the vault
    let transfer_instruction = spl_token::instruction::transfer(
        token_program_info.key,
        source_token_account_info.key,
        destination_token_account_info.key,
        depositor_info.key,
        &[],
        amount,
    )?;
    
    invoke(
        &transfer_instruction,
        &[
            source_token_account_info.clone(),
            destination_token_account_info.clone(),
            depositor_info.clone(),
            token_program_info.clone(),
        ],
    )?;
    
    // Reset reentrancy guard
    vault.reentrancy_guard = false;
    
    // Serialize and store the updated vault data
    vault.serialize(&mut *vault_account_info.data.borrow_mut())?;
    
    msg!("Deposit successful: {} tokens locked until timestamp {}", amount, unlock_time);
    Ok(())
}

// Process withdraw instruction
fn process_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    deposit_id: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let owner_info = next_account_info(account_info_iter)?;
    let vault_account_info = next_account_info(account_info_iter)?;
    let destination_token_account_info = next_account_info(account_info_iter)?;
    let source_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;
    
    // Verify the owner signed the transaction
    if !owner_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify the vault account is owned by the program
    if vault_account_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Load the vault
    let mut vault = Vault::try_from_slice(&vault_account_info.data.borrow())?;
    
    // Check reentrancy guard
    if vault.reentrancy_guard {
        return Err(VaultError::ReentrancyDetected.into());
    }
    
    // Set reentrancy guard
    vault.reentrancy_guard = true;
    
    // Find the deposit
    let deposit_index = vault.deposits.iter().position(|d| d.id == deposit_id)
        .ok_or(VaultError::DepositNotFound)?;
    let deposit = &mut vault.deposits[deposit_index];
    
    // Verify the owner is the depositor
    if deposit.depositor != *owner_info.key {
        return Err(VaultError::UnauthorizedWithdrawal.into());
    }
    
    // Verify the deposit has not been withdrawn
    if deposit.withdrawn {
        return Err(VaultError::AlreadyWithdrawn.into());
    }
    
    // Verify the unlock time has been reached
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    if deposit.unlock_time > clock.unix_timestamp {
        return Err(VaultError::UnlockTimeNotReached.into());
    }
    
    // Mark the deposit as withdrawn
    deposit.withdrawn = true;
    
    // Transfer tokens from the vault to the owner
    let transfer_instruction = spl_token::instruction::transfer(
        token_program_info.key,
        source_token_account_info.key,
        destination_token_account_info.key,
        &vault_account_info.key,
        &[],
        deposit.amount,
    )?;
    
    invoke_signed(
        &transfer_instruction,
        &[
            source_token_account_info.clone(),
            destination_token_account_info.clone(),
            vault_account_info.clone(),
            token_program_info.clone(),
        ],
        &[&[&vault_account_info.key.to_bytes(), &[0]]],
    )?;
    
    // Reset reentrancy guard
    vault.reentrancy_guard = false;
    
    // Serialize and store the updated vault data
    vault.serialize(&mut *vault_account_info.data.borrow_mut())?;
    
    msg!("Withdrawal successful: {} tokens from deposit {}", deposit.amount, deposit_id);
    Ok(())
}

// Process emergency withdraw instruction
fn process_emergency_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    deposit_id: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let emergency_authority_info = next_account_info(account_info_iter)?;
    let vault_account_info = next_account_info(account_info_iter)?;
    let destination_token_account_info = next_account_info(account_info_iter)?;
    let source_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let depositor_info = next_account_info(account_info_iter)?;
    
    // Verify the emergency authority signed the transaction
    if !emergency_authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify the vault account is owned by the program
    if vault_account_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Load the vault
    let mut vault = Vault::try_from_slice(&vault_account_info.data.borrow())?;
    
    // Check reentrancy guard
    if vault.reentrancy_guard {
        return Err(VaultError::ReentrancyDetected.into());
    }
    
    // Set reentrancy guard
    vault.reentrancy_guard = true;
    
    // Verify the emergency authority is authorized
    if vault.emergency_authority.is_none() || vault.emergency_authority.unwrap() != *emergency_authority_info.key {
        return Err(VaultError::UnauthorizedWithdrawal.into());
    }
    
    // Find the deposit
    let deposit_index = vault.deposits.iter().position(|d| d.id == deposit_id)
        .ok_or(VaultError::DepositNotFound)?;
    let deposit = &mut vault.deposits[deposit_index];
    
    // Verify the deposit has not been withdrawn
    if deposit.withdrawn {
        return Err(VaultError::AlreadyWithdrawn.into());
    }
    
    // Verify the depositor account matches the deposit's depositor
    if deposit.depositor != *depositor_info.key {
        return Err(VaultError::UnauthorizedWithdrawal.into());
    }
    
    // Mark the deposit as withdrawn
    deposit.withdrawn = true;
    
    // Transfer tokens from the vault to the depositor
    let transfer_instruction = spl_token::instruction::transfer(
        token_program_info.key,
        source_token_account_info.key,
        destination_token_account_info.key,
        &vault_account_info.key,
        &[],
        deposit.amount,
    )?;
    
    invoke_signed(
        &transfer_instruction,
        &[
            source_token_account_info.clone(),
            destination_token_account_info.clone(),
            vault_account_info.clone(),
            token_program_info.clone(),
        ],
        &[&[&vault_account_info.key.to_bytes(), &[0]]],
    )?;
    
    // Reset reentrancy guard
    vault.reentrancy_guard = false;
    
    // Serialize and store the updated vault data
    vault.serialize(&mut *vault_account_info.data.borrow_mut())?;
    
    msg!("Emergency withdrawal successful: {} tokens from deposit {}", deposit.amount, deposit_id);
    Ok(())
}
