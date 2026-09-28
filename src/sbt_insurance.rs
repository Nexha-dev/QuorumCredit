//! # SBT Insurance Coverage (Issue #1735)
//!
//! Provides a lightweight on-chain insurance layer for SBT holders.  An
//! insured SBT can file a claim against a registered coverage policy; approved
//! claims disburse up to the policy's `coverage_amount` from the contract's
//! slash treasury or yield reserve.
//!
//! ## Insurance lifecycle
//!
//! ```text
//! register_sbt_insurance(admin_signers, sbt_id, holder, coverage_amount)
//!       │
//!       ▼
//!  SbtInsuranceRecord { status: Active, coverage_amount, … }
//!       │
//!       └─ file_sbt_insurance_claim(sbt_id, holder, claim_amount, reason)
//!               │
//!               ▼
//!        claim queued → approve_sbt_insurance_claim(admin_signers, sbt_id)
//!               │
//!               ▼
//!        payout transferred; status → ClaimPaid
//! ```
//!
//! ## Payout source
//!
//! Insurance payouts are sourced from the contract's own token balance (same
//! yield reserve used by voucher rewards).  Admins must ensure the reserve
//! is adequately funded.
//!
//! ## Access control
//!
//! - `register_sbt_insurance` — admin-threshold approval.
//! - `file_sbt_insurance_claim` — signed by `holder`.
//! - `approve_sbt_insurance_claim` — admin-threshold approval.
//! - `get_sbt_insurance` / `get_sbt_insurance_payouts` — public read.

#![allow(unused)]

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, Env, Vec};

use crate::errors::ContractError;
use crate::helpers::{require_admin_approval, require_not_paused, token_client};
use crate::types::{DataKey, PERSISTENT_TTL_TARGET_LEDGERS, PERSISTENT_TTL_THRESHOLD_LEDGERS};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of payout records retained per `sbt_id`.
pub const SBT_INSURANCE_PAYOUT_HISTORY_LIMIT: u32 = 50;

/// Maximum byte length of a claim reason string.
pub const SBT_CLAIM_REASON_MAX_BYTES: u32 = 512;

// ── Data Structures ───────────────────────────────────────────────────────────

/// The status of an SBT insurance policy.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SbtInsuranceStatus {
    /// Policy is active and claims may be filed.
    Active,
    /// A claim has been filed and is awaiting admin approval.
    ClaimPending,
    /// A claim was approved and the payout was disbursed.
    ClaimPaid,
    /// The policy has been cancelled by an admin.
    Cancelled,
}

/// A single insurance payout record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtInsurancePayoutEntry {
    /// Ledger timestamp of the payout.
    pub timestamp: u64,
    /// Amount paid out in stroops.
    pub amount: i128,
    /// Human-readable claim reason provided by the holder.
    pub reason: Bytes,
}

/// The full insurance record for an SBT.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtInsuranceRecord {
    /// The SBT this policy covers.
    pub sbt_id: u64,
    /// The SBT holder (claimant).
    pub holder: Address,
    /// Maximum payout in stroops.
    pub coverage_amount: i128,
    /// Cumulative amount already paid out in stroops.
    pub total_paid_out: i128,
    /// Current policy status.
    pub status: SbtInsuranceStatus,
    /// Pending claim amount (set when `ClaimPending`).
    pub pending_claim_amount: i128,
    /// Pending claim reason.
    pub pending_claim_reason: Bytes,
    /// Ledger timestamp when this policy was registered.
    pub registered_at: u64,
    /// Bounded history of completed payouts.
    pub payouts: Vec<SbtInsurancePayoutEntry>,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn load_record(env: &Env, sbt_id: u64) -> Option<SbtInsuranceRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::SbtInsurance(sbt_id))
}

fn save_record(env: &Env, record: &SbtInsuranceRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::SbtInsurance(record.sbt_id), record);
    env.storage().persistent().extend_ttl(
        &DataKey::SbtInsurance(record.sbt_id),
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Issue #1735 — Register an insurance policy for an SBT.
///
/// Creates a new [`SbtInsuranceRecord`] in `Active` status.  `coverage_amount`
/// is the maximum payout in stroops; it must be > 0.
///
/// Requires admin-threshold approval.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::InvalidAmount`] — `sbt_id` is 0 or `coverage_amount` ≤ 0.
/// - [`ContractError::InvalidStateTransition`] — a policy for this `sbt_id`
///   already exists.
pub fn register_sbt_insurance(
    env: &Env,
    admin_signers: Vec<Address>,
    sbt_id: u64,
    holder: Address,
    coverage_amount: i128,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    require_admin_approval(env, &admin_signers);

    if sbt_id == 0 {
        return Err(ContractError::InvalidAmount);
    }
    if coverage_amount <= 0 {
        return Err(ContractError::InvalidAmount);
    }
    if load_record(env, sbt_id).is_some() {
        return Err(ContractError::InvalidStateTransition);
    }

    let record = SbtInsuranceRecord {
        sbt_id,
        holder: holder.clone(),
        coverage_amount,
        total_paid_out: 0,
        status: SbtInsuranceStatus::Active,
        pending_claim_amount: 0,
        pending_claim_reason: Bytes::new(env),
        registered_at: env.ledger().timestamp(),
        payouts: Vec::new(env),
    };

    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_ins"), symbol_short!("register")),
        (sbt_id, holder, coverage_amount),
    );

    Ok(())
}

/// Issue #1735 — File an insurance claim for an SBT.
///
/// Sets the policy status to `ClaimPending` and records the requested
/// `claim_amount` and `reason`.  Only one pending claim is allowed at a time.
///
/// The `holder` must sign the transaction.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtInsuranceNotFound`] — no policy for `sbt_id`.
/// - [`ContractError::UnauthorizedCaller`] — caller is not the holder.
/// - [`ContractError::InvalidStateTransition`] — policy is not `Active`.
/// - [`ContractError::InvalidAmount`] — `claim_amount` ≤ 0, exceeds remaining
///   coverage, or `reason` exceeds [`SBT_CLAIM_REASON_MAX_BYTES`].
pub fn file_sbt_insurance_claim(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    claim_amount: i128,
    reason: Bytes,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    let mut record =
        load_record(env, sbt_id).ok_or(ContractError::SbtInsuranceNotFound)?;

    if record.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }
    if record.status != SbtInsuranceStatus::Active {
        return Err(ContractError::InvalidStateTransition);
    }
    if claim_amount <= 0 {
        return Err(ContractError::InvalidAmount);
    }
    let remaining_coverage = record.coverage_amount - record.total_paid_out;
    if claim_amount > remaining_coverage {
        return Err(ContractError::InvalidAmount);
    }
    if reason.len() > SBT_CLAIM_REASON_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }

    record.status = SbtInsuranceStatus::ClaimPending;
    record.pending_claim_amount = claim_amount;
    record.pending_claim_reason = reason.clone();

    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_ins"), symbol_short!("claim")),
        (sbt_id, holder, claim_amount),
    );

    Ok(())
}

/// Issue #1735 — Approve a pending insurance claim and disburse the payout.
///
/// Transfers `pending_claim_amount` from the contract to the holder and
/// transitions the policy status to `ClaimPaid`.  Appends a payout entry to
/// the history log.
///
/// Requires admin-threshold approval.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtInsuranceNotFound`] — no policy for `sbt_id`.
/// - [`ContractError::InvalidStateTransition`] — policy is not `ClaimPending`.
pub fn approve_sbt_insurance_claim(
    env: &Env,
    admin_signers: Vec<Address>,
    sbt_id: u64,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    require_admin_approval(env, &admin_signers);

    let mut record =
        load_record(env, sbt_id).ok_or(ContractError::SbtInsuranceNotFound)?;

    if record.status != SbtInsuranceStatus::ClaimPending {
        return Err(ContractError::InvalidStateTransition);
    }

    let payout_amount = record.pending_claim_amount;
    let now = env.ledger().timestamp();

    // Disburse payout from contract balance
    let tok = token_client(env);
    tok.transfer(
        &env.current_contract_address(),
        &record.holder,
        &payout_amount,
    );

    // Record the payout in history
    let payout_entry = SbtInsurancePayoutEntry {
        timestamp: now,
        amount: payout_amount,
        reason: record.pending_claim_reason.clone(),
    };
    record.payouts.push_back(payout_entry);
    while record.payouts.len() > SBT_INSURANCE_PAYOUT_HISTORY_LIMIT {
        record.payouts.pop_front();
    }

    record.total_paid_out = record.total_paid_out.saturating_add(payout_amount);
    record.pending_claim_amount = 0;
    record.pending_claim_reason = Bytes::new(env);
    record.status = SbtInsuranceStatus::ClaimPaid;

    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_ins"), symbol_short!("approved")),
        (sbt_id, record.holder.clone(), payout_amount),
    );

    Ok(())
}

/// Issue #1735 — Cancel an SBT insurance policy.
///
/// Requires admin-threshold approval.  Only `Active` or `ClaimPending`
/// policies may be cancelled.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtInsuranceNotFound`]
/// - [`ContractError::InvalidStateTransition`] — policy is already paid or cancelled.
pub fn cancel_sbt_insurance(
    env: &Env,
    admin_signers: Vec<Address>,
    sbt_id: u64,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    require_admin_approval(env, &admin_signers);

    let mut record =
        load_record(env, sbt_id).ok_or(ContractError::SbtInsuranceNotFound)?;

    if matches!(
        record.status,
        SbtInsuranceStatus::ClaimPaid | SbtInsuranceStatus::Cancelled
    ) {
        return Err(ContractError::InvalidStateTransition);
    }

    record.status = SbtInsuranceStatus::Cancelled;
    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_ins"), symbol_short!("cancel")),
        (sbt_id, record.holder),
    );

    Ok(())
}

/// Issue #1735 — Return the full insurance record for `sbt_id`.
pub fn get_sbt_insurance(
    env: &Env,
    sbt_id: u64,
) -> Result<SbtInsuranceRecord, ContractError> {
    load_record(env, sbt_id).ok_or(ContractError::SbtInsuranceNotFound)
}

/// Issue #1735 — Return the payout history for `sbt_id`.
pub fn get_sbt_insurance_payouts(
    env: &Env,
    sbt_id: u64,
) -> Result<Vec<SbtInsurancePayoutEntry>, ContractError> {
    let record = load_record(env, sbt_id).ok_or(ContractError::SbtInsuranceNotFound)?;
    Ok(record.payouts)
}
