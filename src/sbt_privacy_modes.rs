//! # SBT Privacy Modes (Issue #1734)
//!
//! Extends Soulbound Token visibility from binary (public/private) to three
//! graduated privacy levels: `Public`, `FriendsOnly`, and `Private`.
//!
//! ## Privacy levels
//!
//! | Level         | Who can view details                                  |
//! |---------------|-------------------------------------------------------|
//! | `Public`      | Anyone — no auth required                             |
//! | `FriendsOnly` | Addresses explicitly added to the holder's allow-list |
//! | `Private`     | Only the SBT holder themselves                        |
//!
//! ## Lifecycle
//!
//! ```text
//! set_sbt_privacy_mode(sbt_id, holder, mode)
//!       │
//!       ▼
//!  SbtPrivacyRecord { sbt_id, holder, mode, history }
//!       │
//!       ├─ add_sbt_friend(sbt_id, holder, friend)     ← FriendsOnly allow-list
//!       ├─ remove_sbt_friend(sbt_id, holder, friend)
//!       └─ check_sbt_access(sbt_id, viewer) → bool
//! ```
//!
//! ## History tracking
//!
//! Every call to `set_sbt_privacy_mode` appends a
//! [`SbtPrivacyHistoryEntry`] so that the full change log can be audited
//! off-chain.  History is capped at [`SBT_PRIVACY_HISTORY_LIMIT`] entries.
//!
//! ## Access control
//!
//! - `set_sbt_privacy_mode`, `add_sbt_friend`, `remove_sbt_friend` all require
//!   the transaction to be signed by the `holder` of the SBT.
//! - `check_sbt_access` is a read-only view requiring no auth.

#![allow(unused)]

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

use crate::errors::ContractError;
use crate::helpers::require_not_paused;
use crate::types::{DataKey, PERSISTENT_TTL_TARGET_LEDGERS, PERSISTENT_TTL_THRESHOLD_LEDGERS};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum entries kept in the privacy-mode history log.
pub const SBT_PRIVACY_HISTORY_LIMIT: u32 = 100;

/// Maximum number of addresses that can be in a single holder's friends list.
pub const SBT_FRIENDS_MAX: u32 = 200;

// ── Data Structures ───────────────────────────────────────────────────────────

/// The three supported privacy levels.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivacyMode {
    /// Fully visible to anyone without authentication.
    Public,
    /// Visible only to addresses on the holder's friends list.
    FriendsOnly,
    /// Visible only to the holder themselves.
    Private,
}

/// A single entry in the privacy-mode history log.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtPrivacyHistoryEntry {
    /// Ledger timestamp of the change.
    pub timestamp: u64,
    /// The mode that was set at this point.
    pub mode: PrivacyMode,
}

/// The full privacy record for an SBT.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtPrivacyRecord {
    /// The SBT this record belongs to.
    pub sbt_id: u64,
    /// The holder (owner) of the SBT.
    pub holder: Address,
    /// Current privacy mode.
    pub mode: PrivacyMode,
    /// Addresses explicitly allowed to view under `FriendsOnly`.
    pub friends: Vec<Address>,
    /// Bounded history of previous mode changes.
    pub history: Vec<SbtPrivacyHistoryEntry>,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn load_record(env: &Env, sbt_id: u64) -> Option<SbtPrivacyRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::SbtPrivacy(sbt_id))
}

fn save_record(env: &Env, record: &SbtPrivacyRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::SbtPrivacy(record.sbt_id), record);
    env.storage().persistent().extend_ttl(
        &DataKey::SbtPrivacy(record.sbt_id),
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Issue #1734 — Set the privacy mode for an SBT.
///
/// The `holder` must sign the transaction.  On the first call for a given
/// `sbt_id` a new [`SbtPrivacyRecord`] is created; subsequent calls update
/// the mode and append to the history log.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::InvalidAmount`] — `sbt_id` is 0.
/// - [`ContractError::UnauthorizedCaller`] — caller is not the recorded holder
///   (only enforced after the record already exists; the first call establishes
///   the holder).
pub fn set_sbt_privacy_mode(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    mode: PrivacyMode,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    if sbt_id == 0 {
        return Err(ContractError::InvalidAmount);
    }

    let now = env.ledger().timestamp();

    let mut record = load_record(env, sbt_id).unwrap_or_else(|| SbtPrivacyRecord {
        sbt_id,
        holder: holder.clone(),
        mode: PrivacyMode::Public,
        friends: Vec::new(env),
        history: Vec::new(env),
    });

    // After creation, only the holder may change settings
    if record.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }

    // Append current mode to history before updating
    let entry = SbtPrivacyHistoryEntry {
        timestamp: now,
        mode: record.mode.clone(),
    };
    record.history.push_back(entry);
    while record.history.len() > SBT_PRIVACY_HISTORY_LIMIT {
        record.history.pop_front();
    }

    record.mode = mode.clone();
    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_prv"), symbol_short!("set_mode")),
        (sbt_id, holder, mode),
    );

    Ok(())
}

/// Issue #1734 — Add an address to the SBT's friends allow-list.
///
/// Only meaningful when the mode is (or will be) `FriendsOnly`.  The record
/// must already exist; call [`set_sbt_privacy_mode`] first.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtPrivacyNotFound`] — no privacy record for `sbt_id`.
/// - [`ContractError::UnauthorizedCaller`] — caller is not the holder.
/// - [`ContractError::InvalidAmount`] — friends list is full or `friend` is
///   the holder themselves.
/// - [`ContractError::DuplicateVouch`] — `friend` is already in the list.
pub fn add_sbt_friend(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    friend: Address,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    let mut record =
        load_record(env, sbt_id).ok_or(ContractError::SbtPrivacyNotFound)?;

    if record.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }
    if friend == holder {
        return Err(ContractError::InvalidAmount);
    }
    if record.friends.len() >= SBT_FRIENDS_MAX {
        return Err(ContractError::InvalidAmount);
    }
    if record.friends.iter().any(|f| f == friend) {
        return Err(ContractError::DuplicateVouch);
    }

    record.friends.push_back(friend.clone());
    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_prv"), symbol_short!("add_frnd")),
        (sbt_id, holder, friend),
    );

    Ok(())
}

/// Issue #1734 — Remove an address from the SBT's friends allow-list.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtPrivacyNotFound`] — no privacy record for `sbt_id`.
/// - [`ContractError::UnauthorizedCaller`] — caller is not the holder.
/// - [`ContractError::VoucherNotFound`] — `friend` is not in the list.
pub fn remove_sbt_friend(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    friend: Address,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    let mut record =
        load_record(env, sbt_id).ok_or(ContractError::SbtPrivacyNotFound)?;

    if record.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }

    let mut new_friends: Vec<Address> = Vec::new(env);
    let mut found = false;
    for f in record.friends.iter() {
        if f == friend {
            found = true;
        } else {
            new_friends.push_back(f);
        }
    }
    if !found {
        return Err(ContractError::VoucherNotFound);
    }

    record.friends = new_friends;
    save_record(env, &record);

    env.events().publish(
        (symbol_short!("sbt_prv"), symbol_short!("rm_frnd")),
        (sbt_id, holder, friend),
    );

    Ok(())
}

/// Issue #1734 — Check whether `viewer` has access to the SBT.
///
/// Returns `true` when:
/// - mode is `Public`, OR
/// - mode is `FriendsOnly` and `viewer` is in the friends list, OR
/// - mode is `Private` and `viewer` is the holder.
///
/// Returns `false` if no privacy record exists (defaults to public).
pub fn check_sbt_access(env: &Env, sbt_id: u64, viewer: Address) -> bool {
    let record = match load_record(env, sbt_id) {
        Some(r) => r,
        None => return true, // no record → treat as public
    };

    match record.mode {
        PrivacyMode::Public => true,
        PrivacyMode::FriendsOnly => {
            viewer == record.holder || record.friends.iter().any(|f| f == viewer)
        }
        PrivacyMode::Private => viewer == record.holder,
    }
}

/// Issue #1734 — Return the full privacy record for `sbt_id`.
///
/// # Errors
/// - [`ContractError::SbtPrivacyNotFound`] — no record exists for `sbt_id`.
pub fn get_sbt_privacy_record(
    env: &Env,
    sbt_id: u64,
) -> Result<SbtPrivacyRecord, ContractError> {
    load_record(env, sbt_id).ok_or(ContractError::SbtPrivacyNotFound)
}

/// Issue #1734 — Return the privacy-mode history for `sbt_id`.
pub fn get_sbt_privacy_history(
    env: &Env,
    sbt_id: u64,
) -> Result<Vec<SbtPrivacyHistoryEntry>, ContractError> {
    let record = load_record(env, sbt_id).ok_or(ContractError::SbtPrivacyNotFound)?;
    Ok(record.history)
}
