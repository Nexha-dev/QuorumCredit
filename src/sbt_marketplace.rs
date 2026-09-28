//! # SBT Marketplace Integration (Issue #1736)
//!
//! Enables Soulbound Token holders to list their credentials in a
//! discoverable on-chain marketplace directory.  Because SBTs are
//! non-transferable, a "listing" here means making the credential's metadata
//! publicly discoverable and searchable — not offering it for sale.
//!
//! ## Listing lifecycle
//!
//! ```text
//! list_sbt_in_marketplace(sbt_id, holder, metadata)
//!       │
//!       ▼
//!  SbtMarketplaceListing { sbt_id, holder, metadata, active: true, … }
//!       │
//!       ├─ update_sbt_marketplace_listing(sbt_id, holder, new_metadata)
//!       ├─ remove_sbt_marketplace_listing(sbt_id, holder)
//!       └─ search_sbt_marketplace(query_tag) → Vec<u64>  ← sbt_ids
//! ```
//!
//! ## Search and filtering
//!
//! A single `tag` byte-string is stored per listing to support basic
//! categorical filtering.  Off-chain indexers may index `metadata` for full
//! text search; on-chain, `search_sbt_marketplace` provides an exact-match
//! filter over the `tag` field across the global listing index.
//!
//! The global listing index is a bounded ring-buffer of `sbt_id` values
//! capped at [`SBT_MARKETPLACE_INDEX_LIMIT`].
//!
//! ## Statistics
//!
//! `get_sbt_marketplace_stats` returns protocol-wide counters tracking total
//! listings created, currently active, and total removed.
//!
//! ## Access control
//!
//! - `list_sbt_in_marketplace`, `update_sbt_marketplace_listing`,
//!   `remove_sbt_marketplace_listing` — signed by `holder`.
//! - All read operations — public.

#![allow(unused)]

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, Env, Vec};

use crate::errors::ContractError;
use crate::helpers::require_not_paused;
use crate::types::{DataKey, PERSISTENT_TTL_TARGET_LEDGERS, PERSISTENT_TTL_THRESHOLD_LEDGERS};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum byte length of listing metadata.
pub const SBT_LISTING_METADATA_MAX_BYTES: u32 = 2_048;

/// Maximum byte length of a listing tag.
pub const SBT_LISTING_TAG_MAX_BYTES: u32 = 64;

/// Maximum number of `sbt_id`s tracked in the global marketplace index.
pub const SBT_MARKETPLACE_INDEX_LIMIT: u32 = 10_000;

// ── Data Structures ───────────────────────────────────────────────────────────

/// A single SBT marketplace listing.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtMarketplaceListing {
    /// The SBT being listed.
    pub sbt_id: u64,
    /// The holder (owner) of the SBT.
    pub holder: Address,
    /// Arbitrary metadata payload (e.g. JSON or IPFS CID).
    pub metadata: Bytes,
    /// Short categorical tag for on-chain filtering (e.g. b"identity").
    pub tag: Bytes,
    /// Whether the listing is currently active.
    pub active: bool,
    /// Ledger timestamp when the listing was created.
    pub listed_at: u64,
    /// Ledger timestamp of the most recent metadata update.
    pub updated_at: u64,
}

/// Protocol-wide marketplace statistics.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SbtMarketplaceStats {
    /// Total number of listings ever created.
    pub total_listed: u64,
    /// Number of currently active listings.
    pub active_count: u64,
    /// Total number of listings that have been removed.
    pub total_removed: u64,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn load_listing(env: &Env, sbt_id: u64) -> Option<SbtMarketplaceListing> {
    env.storage()
        .persistent()
        .get(&DataKey::SbtMarketplaceListing(sbt_id))
}

fn save_listing(env: &Env, listing: &SbtMarketplaceListing) {
    env.storage()
        .persistent()
        .set(&DataKey::SbtMarketplaceListing(listing.sbt_id), listing);
    env.storage().persistent().extend_ttl(
        &DataKey::SbtMarketplaceListing(listing.sbt_id),
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

fn load_index(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::SbtMarketplaceIndex)
        .unwrap_or_else(|| Vec::new(env))
}

fn save_index(env: &Env, index: &Vec<u64>) {
    env.storage()
        .persistent()
        .set(&DataKey::SbtMarketplaceIndex, index);
    env.storage().persistent().extend_ttl(
        &DataKey::SbtMarketplaceIndex,
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

fn load_stats(env: &Env) -> SbtMarketplaceStats {
    env.storage()
        .persistent()
        .get(&DataKey::SbtMarketplaceStats)
        .unwrap_or(SbtMarketplaceStats {
            total_listed: 0,
            active_count: 0,
            total_removed: 0,
        })
}

fn save_stats(env: &Env, stats: &SbtMarketplaceStats) {
    env.storage()
        .persistent()
        .set(&DataKey::SbtMarketplaceStats, stats);
    env.storage().persistent().extend_ttl(
        &DataKey::SbtMarketplaceStats,
        PERSISTENT_TTL_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Issue #1736 — Create a new marketplace listing for an SBT.
///
/// The `holder` must sign the transaction.  `metadata` holds discoverable
/// information about the credential; `tag` is a short category label used
/// for on-chain filtering.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::InvalidAmount`] — `sbt_id` is 0, or `metadata` / `tag`
///   exceed their respective byte-length limits.
/// - [`ContractError::InvalidStateTransition`] — an active listing for
///   `sbt_id` already exists.
pub fn list_sbt_in_marketplace(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    metadata: Bytes,
    tag: Bytes,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    if sbt_id == 0 {
        return Err(ContractError::InvalidAmount);
    }
    if metadata.len() > SBT_LISTING_METADATA_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }
    if tag.len() > SBT_LISTING_TAG_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }

    // Reject if an active listing already exists
    if let Some(existing) = load_listing(env, sbt_id) {
        if existing.active {
            return Err(ContractError::InvalidStateTransition);
        }
    }

    let now = env.ledger().timestamp();

    let listing = SbtMarketplaceListing {
        sbt_id,
        holder: holder.clone(),
        metadata: metadata.clone(),
        tag: tag.clone(),
        active: true,
        listed_at: now,
        updated_at: now,
    };

    save_listing(env, &listing);

    // Update global index (append, cap at limit)
    let mut index = load_index(env);
    if !index.iter().any(|id| id == sbt_id) {
        if index.len() >= SBT_MARKETPLACE_INDEX_LIMIT {
            index.pop_front(); // evict oldest entry
        }
        index.push_back(sbt_id);
        save_index(env, &index);
    }

    // Update stats
    let mut stats = load_stats(env);
    stats.total_listed += 1;
    stats.active_count += 1;
    save_stats(env, &stats);

    env.events().publish(
        (symbol_short!("sbt_mkt"), symbol_short!("list")),
        (sbt_id, holder, tag),
    );

    Ok(())
}

/// Issue #1736 — Update the metadata and/or tag of an existing active listing.
///
/// The `holder` must sign the transaction.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtMarketplaceListingNotFound`] — no listing for `sbt_id`.
/// - [`ContractError::UnauthorizedCaller`] — caller is not the holder.
/// - [`ContractError::InvalidStateTransition`] — listing is not active.
/// - [`ContractError::InvalidAmount`] — `metadata` / `tag` exceed limits.
pub fn update_sbt_marketplace_listing(
    env: &Env,
    holder: Address,
    sbt_id: u64,
    metadata: Bytes,
    tag: Bytes,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    if metadata.len() > SBT_LISTING_METADATA_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }
    if tag.len() > SBT_LISTING_TAG_MAX_BYTES {
        return Err(ContractError::InvalidAmount);
    }

    let mut listing =
        load_listing(env, sbt_id).ok_or(ContractError::SbtMarketplaceListingNotFound)?;

    if listing.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }
    if !listing.active {
        return Err(ContractError::InvalidStateTransition);
    }

    listing.metadata = metadata.clone();
    listing.tag = tag.clone();
    listing.updated_at = env.ledger().timestamp();

    save_listing(env, &listing);

    env.events().publish(
        (symbol_short!("sbt_mkt"), symbol_short!("update")),
        (sbt_id, holder, tag),
    );

    Ok(())
}

/// Issue #1736 — Remove (de-list) an SBT from the marketplace.
///
/// Sets `active = false` on the listing record and decrements the active
/// counter in stats.  The listing record is retained for historical purposes.
///
/// # Errors
/// - [`ContractError::ContractPaused`]
/// - [`ContractError::SbtMarketplaceListingNotFound`]
/// - [`ContractError::UnauthorizedCaller`] — caller is not the holder.
/// - [`ContractError::InvalidStateTransition`] — listing is already inactive.
pub fn remove_sbt_marketplace_listing(
    env: &Env,
    holder: Address,
    sbt_id: u64,
) -> Result<(), ContractError> {
    require_not_paused(env)?;
    holder.require_auth();

    let mut listing =
        load_listing(env, sbt_id).ok_or(ContractError::SbtMarketplaceListingNotFound)?;

    if listing.holder != holder {
        return Err(ContractError::UnauthorizedCaller);
    }
    if !listing.active {
        return Err(ContractError::InvalidStateTransition);
    }

    listing.active = false;
    save_listing(env, &listing);

    // Update stats
    let mut stats = load_stats(env);
    stats.active_count = stats.active_count.saturating_sub(1);
    stats.total_removed += 1;
    save_stats(env, &stats);

    env.events().publish(
        (symbol_short!("sbt_mkt"), symbol_short!("remove")),
        (sbt_id, holder),
    );

    Ok(())
}

/// Issue #1736 — Return all active listing `sbt_id`s whose `tag` matches
/// `query_tag` exactly.
///
/// This performs a linear scan of the global index and is intended for
/// on-chain eligibility gates.  For full-text or complex queries, use an
/// off-chain indexer.
pub fn search_sbt_marketplace(env: &Env, query_tag: Bytes) -> Vec<u64> {
    let index = load_index(env);
    let mut results: Vec<u64> = Vec::new(env);

    for sbt_id in index.iter() {
        if let Some(listing) = load_listing(env, sbt_id) {
            if listing.active && listing.tag == query_tag {
                results.push_back(sbt_id);
            }
        }
    }

    results
}

/// Issue #1736 — Return the marketplace listing for `sbt_id`.
pub fn get_sbt_marketplace_listing(
    env: &Env,
    sbt_id: u64,
) -> Result<SbtMarketplaceListing, ContractError> {
    load_listing(env, sbt_id).ok_or(ContractError::SbtMarketplaceListingNotFound)
}

/// Issue #1736 — Return protocol-wide marketplace statistics.
pub fn get_sbt_marketplace_stats(env: &Env) -> SbtMarketplaceStats {
    load_stats(env)
}
