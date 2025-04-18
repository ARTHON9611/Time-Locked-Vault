---

# ⏳ Time-Locked Savings Vault

A secure, modular smart contract for a **Time-Locked Savings Vault** built on the **Arch Network** using **Rust** and the **Solana Program Library**. This vault acts like a programmable digital piggy bank — allowing users to lock funds until a future unlock time.

---

## 🚀 Features

### 🔐 Core Functionalities
- **Multi-Token Support**: Lock SOL, SPL tokens, and wrapped assets with custom unlock timestamps.
- **Time-Locked Logic**: Funds are inaccessible until the specified unlock time.
- **Multiple Deposits**: Users can create multiple deposits with different unlock times and token types.
- **Secure Withdrawals**: Only the depositor can withdraw after the unlock time.

### 🛡️ Security Features
- **Reentrancy Protection**: Guard flag ensures safe execution.
- **Access Control**: Strict depositor identity verification.
- **Timestamp Validation**: Prevents manipulation of unlock times.
- **Edge Case Handling**: Graceful handling of zero amounts, past times, etc.

### 🎁 Bonus Features
- **Emergency Unlock**: Optional multisig/DAO-based unlock mechanism.
- **Deposit Tagging**: Label deposits with tags like `"Rent"`, `"Vacation"`, `"Long-term"` for clarity.

---

## 🏗️ Contract Architecture

Built with **Rust** and **Solana Program Library** using a modular design.

### 📦 Data Structures
- **Vault**: Stores vault metadata (owner, deposits, guard flag, etc.)
- **Deposit**: Tracks each deposit's ID, amount, unlock time, tag, and more.

### 🧾 Instructions
- `CreateVault`: Initializes a new vault.
- `Deposit`: Locks tokens with a specific unlock time.
- `Withdraw`: Allows token retrieval after unlock.
- `EmergencyWithdraw`: Withdraws funds via emergency authority (e.g., multisig).

### ❌ Error Handling
Handles cases like:
- Unlock time not reached
- Invalid deposit amount or ID
- Reentrancy attempts
- Unauthorized withdrawals
- Math overflows

---

## 📘 Usage Guide

### 🔧 Create a Vault

```rust
let instruction = VaultInstruction::CreateVault;
let accounts = vec![
    AccountMeta::new(owner.pubkey(), true),
    AccountMeta::new(vault_account.pubkey(), false),
    AccountMeta::new_readonly(system_program::ID, false),
];
```

### 💰 Deposit Tokens

```rust
let unlock_time = current_unix_timestamp + 30 * 24 * 60 * 60; // 30 days
let tag = b"Vacation Savings\0\0\0\0\0\0\0\0\0\0\0\0";

let instruction = VaultInstruction::Deposit {
    amount: 100,
    unlock_time,
    tag: *tag,
};

let accounts = vec![
    AccountMeta::new(depositor.pubkey(), true),
    AccountMeta::new(vault_account.pubkey(), false),
    AccountMeta::new(source_token_account.pubkey(), false),
    AccountMeta::new(destination_token_account.pubkey(), false),
    AccountMeta::new_readonly(spl_token::id(), false),
    AccountMeta::new_readonly(system_program::ID, false),
    AccountMeta::new_readonly(sysvar::clock::id(), false),
];
```

### 🔓 Withdraw Tokens

```rust
let instruction = VaultInstruction::Withdraw {
    deposit_id: 0,
};
```

### 🆘 Emergency Withdrawal

```rust
let instruction = VaultInstruction::EmergencyWithdraw {
    deposit_id: 0,
};

let accounts = vec![
    AccountMeta::new(emergency_authority.pubkey(), true),
    AccountMeta::new(vault_account.pubkey(), false),
    AccountMeta::new(destination_token_account.pubkey(), false),
    AccountMeta::new(source_token_account.pubkey(), false),
    AccountMeta::new_readonly(spl_token::id(), false),
    AccountMeta::new_readonly(depositor.pubkey(), false),
];
```

---

## ⚙️ Deployment

1. **Clone the Repo**
   ```bash
   git clone https://github.com/your-username/time-locked-vault.git
   cd time-locked-vault
   ```

2. **Build**
   ```bash
   cargo build-bpf
   ```

3. **Deploy**
   ```bash
   solana program deploy --program-id <KEYPAIR_PATH> target/deploy/time_locked_vault.so
   ```

4. **Initialize Vault**
   ```bash
   solana program call <PROGRAM_ID> CreateVault --keypair <OWNER_KEYPAIR>
   ```

---

## 🧪 Testing

Includes unit tests for:
- Vault creation
- Deposits & withdrawals
- Edge cases (e.g., past unlock times)
- Security checks (e.g., reentrancy)

Run tests:
```bash
cargo test
```

---

## 🛠️ Future Enhancements

- 💹 **Interest Accrual** via yield protocols
- 🏷️ **Advanced Tagging System** with metadata
- 🌐 **Frontend Integration** using React, TailwindCSS, and Solana Wallet Adapter

---

## 📄 License

This project is licensed under the [MIT License](./LICENSE).

---
