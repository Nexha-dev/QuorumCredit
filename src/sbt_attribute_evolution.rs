//! # SBT Attribute Evolution (Issue #1733)
//!
//! Enables Soulbound Token attributes to evolve over time.  Each attribute is
//! a `Bytes` key mapped to a `Bytes` value.  Every mutation is appended to a
//! bounded history log, and any previous version can be reinstated via rollback.
//!
//! ## Lifecycle
//!
//! ```text
//! evolve_sbt_attribute(sbt_id, attribute, new_value)
//!       │
//!       ▼
//!  SbtAttributeRecord { current_value, version, history }
//!       │
//!       └─ rollback_sbt_attribute(sbt_id, attribute, target_version)
//!               └─ restores that version's value as the new current value
//! ```
//!
//! ## Versioning
//!
//! Versions are 1-based monotonically increasing counters stored per
//! `(sbt_id, attribute)`.  The current version is always the latest evolution.
//! History entries keep `(version, value, timestamp)` so that off-chain tools
//! can reconstruct the full change log.
//!
//! ## Access control
//!
//! `evolve_sbt_attribute` and `rollback_sbt_attribute` require admin-threshold
//! approval to prevent arbitrary mutation of on-chain credentials.

#![allow(unused)]

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, Env, Vec};

use crate::errors::ContractError;
use crate::helpers::{require_admin_approval, require_not_paused};
use crate::types::{DataKey, PERSISTENT_TTL_TARGET_LEDGERS, PERSISTENT_TTL_THRESHOLD_LEDGERS};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of historical attribute versions retained per
/// `(sbt_id, attribute)` key.
pub const SBT_ATTRIBUTE_HISTORY_LIMIT: u32 = 50;

/// Maximum byte length of an attribute key.
pub const SBT_ATTRIBUTE_KEY_MAX_BYTES: u32 = 128;

/// Maximum byte length of an attribute value.
pub const SBT_ATTRIBUTE_VALUE_MAX_BYTES: u32 = 1_024;

// ── Data Structures ───────────────────────────────────────────────────────────

/// A single historical snapshot of an attribute value.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtAttributeHistoryEntry {
    /// The version number assigned when this value was written.
    pub version: u32,
    /// The attribute value at this version.
    pub value: Bytes,
    /// Ledger timestamp when this version was written.
    pub timestamp: u64,
}

/// The live state of a single versioned SBT attribute.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtAttributeRecord {
    /// The SBT this attribute belongs to.
    pub sbt_id: u64,
    /// The attribute key (arbitrary bytes, e.g. UTF-8 string).
    pub attribute: Bytes,
    /// Current attribute value.
    pub current_value: Bytes,
    /// Monotonically increasing version counter.  Starts at 1.
    pub version: u32,
    /// Bounded history of previous versions, oldest-first.
    pub history: Vec<SbtAttributeHistoryEntry>,
}

// ── Storage key helpers ───────────────────────────────────────────────────────

fn attr_key(sbt_id: u64, attribute: &Bytes) -> DataKey {
    DataKey::SbtAttribute(sbt_id, attribute.clone())
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn load_record(env: &Env, sbt_id: u64, attribute: &Bytes) -> Option<SbtAttributeRecord> {
    env.storage()
        .persistent()
        .get(&attr_key(sbt_id, attribute))
}

fn save_record(env: &Env, record: &SbtAttributeRecord) {
    let key = attr_key(record.sbt_id, &record.attribute);
    env.storage().persistent().set(&key, record);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Issue #1733 — Evolve an SBT attribute to a new value.
///
/// If this is the first time the attribute is written, `version` starts at 1.
/// Subsequent calls increment the version and append the previous value to the
/// history log.  History is capped at [`SBT_ATTRIBUTE_HISTORY_LIMIT`] entries
/// (oldest entries are dropped when the cap is reached).
///
/// Requires admin-threshold approval.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::InvalidAmount`] — `sbt_id` is 0, or `attribute` / `new_value`
///   exceed their respective byte-length limits.
pub fn evolve_sbt_attribute(
    env: &Env,
    admin_signers: Vec<Address>,
    sbt_id: u64,
    attribute: Bytes,
    new_value: Bytes,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    require_admin_approval(env, &admin_signers);

    if sbt_id == 0 {
        return Err(ContractError::InvalidAmount);
    }
    if attribute.len() == 0 || attribute.len() > SBT_ATTRIBUTE_KEY_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }
    if new_value.len() > SBT_ATTRIBUTE_VALUE_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }

    let now = env.ledger().timestamp();

    let mut record = load_record(env, sbt_id, &attribute).unwrap_or_else(|| SbtAttributeRecord {
        sbt_id,
        attribute: attribute.clone(),
        current_value: Bytes::new(env),
        version: 0,
        history: Vec::new(env),
    });

    // Archive current value as a history entry (skip on first write when version == 0)
    if record.version > 0 {
        let entry = SbtAttributeHistoryEntry {
            version: record.version,
            value: record.current_value.clone(),
            timestamp: now,
        };
        record.history.push_back(entry);
        // Trim history to the configured cap
        while record.history.len() > SBT_ATTRIBUTE_HISTORY_LIMIT {
            record.history.pop_front();
        }
    }

    record.version += 1;
    record.current_value = new_value.clone();

    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_att"), symbol_short!("evolve")),
        (sbt_id, attribute, record.version, new_value),
    );

    Ok(())
}

/// Issue #1733 — Rollback an SBT attribute to a specific historical version.
///
/// The `target_version` must exist in the history log.  On success the
/// attribute's `current_value` is restored to that version's value and the
/// `version` counter is incremented (the rollback itself creates a new version
/// entry in the log to maintain auditability).
///
/// Requires admin-threshold approval.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtAttributeNotFound`] — no attribute record exists for
///   `(sbt_id, attribute)`.
/// - [`ContractError::SbtAttributeVersionNotFound`] — `target_version` not in history.
/// - [`ContractError::InvalidAmount`] — `sbt_id` is 0.
pub fn rollback_sbt_attribute(
    env: &Env,
    admin_signers: Vec<Address>,
    sbt_id: u64,
    attribute: Bytes,
    target_version: u32,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    require_admin_approval(env, &admin_signers);

    if sbt_id == 0 {
        return Err(ContractError::InvalidAmount);
    }

    let mut record =
        load_record(env, sbt_id, &attribute).ok_or(ContractError::SbtAttributeNotFound)?;

    // Locate the target version in history
    let target_value: Bytes = record
        .history
        .iter()
        .find(|e| e.version == target_version)
        .map(|e| e.value.clone())
        .ok_or(ContractError::SbtAttributeVersionNotFound)?;

    let now = env.ledger().timestamp();

    // Archive current value
    let entry = SbtAttributeHistoryEntry {
        version: record.version,
        value: record.current_value.clone(),
        timestamp: now,
    };
    record.history.push_back(entry);
    while record.history.len() > SBT_ATTRIBUTE_HISTORY_LIMIT {
        record.history.pop_front();
    }

    record.version += 1;
    record.current_value = target_value.clone();

    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_att"), symbol_short!("rollback")),
        (sbt_id, attribute, target_version, record.version),
    );

    Ok(())
}

/// Issue #1733 — Return the current attribute record for `(sbt_id, attribute)`.
pub fn get_sbt_attribute(
    env: &Env,
    sbt_id: u64,
    attribute: Bytes,
) -> Result<SbtAttributeRecord, ContractError> {
    load_record(env, sbt_id, &attribute).ok_or(ContractError::SbtAttributeNotFound)
}

/// Issue #1733 — Return the full version history for `(sbt_id, attribute)`.
pub fn get_sbt_attribute_history(
    env: &Env,
    sbt_id: u64,
    attribute: Bytes,
) -> Result<Vec<SbtAttributeHistoryEntry>, ContractError> {
    let record =
        load_record(env, sbt_id, &attribute).ok_or(ContractError::SbtAttributeNotFound)?;
    Ok(record.history)
}
