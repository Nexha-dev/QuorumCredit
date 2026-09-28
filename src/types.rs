#![allow(unused)]

//! # Stroop Unit Convention
//!
//! **All monetary amounts in this contract are denominated in stroops.**
//!
//! | Unit  | Value                      |
//! |-------|----------------------------|
//! | 1 XLM | 10,000,000 stroops         |
//! | 1 stroop | 0.0000001 XLM           |
//!
//! This applies to every `i128` field or parameter that represents a token
//! amount (stakes, loan principals, yield, fees, minimums, etc.).
//! When displaying values to end-users, divide by `10_000_000` to convert
//! to XLM. When accepting user input in XLM, multiply by `10_000_000`
//! before passing to contract functions.

use soroban_sdk::{contracttype, Address, Bytes, BytesN, String, Symbol, Vec};

use crate::interest_rate_options::OptionType;
use crate::reputation_nft::BadgeType;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Yield earned by vouchers on full repayment, in basis points (200 = 2%).
pub const DEFAULT_YIELD_BPS: i128 = 200;
/// Fraction of stake burned when a borrower defaults, in basis points (5000 = 50%).
pub const DEFAULT_SLASH_BPS: i128 = 5000;
/// Basis-point denominator (10_000 = 100%).
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Minimum stake amount, in stroops (50 stroops), required for non-zero yield at
/// the default 2% rate. Amounts below this truncate to zero yield.
/// 1 XLM = 10,000,000 stroops.
pub const DEFAULT_MIN_YIELD_STAKE: i128 = 50;
/// Referral bonus paid to the referrer on full repayment, in basis points.
/// Issue #1247 specifies 10% of the referrer's first interest earned (1000 bps).
pub const DEFAULT_REFERRAL_BONUS_BPS: u32 = 1000; // 10% of first loan interest
/// Minimum age of a vouch before it can be used for a loan, in seconds (60 = 1 minute).
pub const MIN_VOUCH_AGE: u64 = 60; // 1 minute
/// Default minimum vouch age before loan eligibility, in seconds (24 hours).
pub const DEFAULT_MIN_VOUCH_AGE_SECS: u64 = 24 * 60 * 60;
/// Default maximum number of distinct vouchers per borrower.
pub const DEFAULT_MAX_VOUCHERS: u32 = 100;
/// Default minimum loan amount, in stroops (100,000 stroops = 0.01 XLM).
/// 1 XLM = 10,000,000 stroops.
pub const DEFAULT_MIN_LOAN_AMOUNT: i128 = 100_000;
/// Default loan duration, in seconds (30 days).
pub const DEFAULT_LOAN_DURATION: u64 = 30 * 24 * 60 * 60;
/// Default payment grace period after a suspension, in seconds (3 days).
pub const PAYMENT_GRACE_PERIOD: u64 = 3 * 24 * 60 * 60;
/// Default maximum loan-to-stake ratio (150 = 150% — loan ≤ 1.5× total staked).
pub const DEFAULT_MAX_LOAN_TO_STAKE_RATIO: u32 = 150;
/// Default maximum loan-to-collateral ratio (50_000 = 50% — loan ≤ 0.5× total stake).
pub const DEFAULT_MAX_LOAN_TO_COLLATERAL_RATIO: u32 = 50_000;
/// Minimum elapsed time between vouch calls from the same address, in seconds (24 hours).
pub const DEFAULT_VOUCH_COOLDOWN_SECS: u64 = 24 * 60 * 60; // 24 hours
/// Default maximum number of vouchers that may back a single borrower.
pub const DEFAULT_MAX_VOUCHERS_PER_BORROWER: u32 = 50;
/// Default timelock delay for config changes, in seconds (7 days).
pub const CONFIG_TIMELOCK_SECONDS: u64 = 7 * 24 * 60 * 60;
/// Issue #1146: maximum number of pending entries a single borrower's
/// withdrawal queue may hold. Requests beyond this cap are rejected
/// (`ContractError::WithdrawalQueueFull`) instead of growing the queue's
/// persistent-storage Vec without bound.
pub const MAX_WITHDRAWAL_QUEUE_SIZE: u32 = 200;
/// Issue #1146: target size of the "hot" per-(borrower, voucher, token)
/// vouch-history window kept after an archival cutover.
pub const MAX_HOT_VOUCH_HISTORY_ENTRIES: u32 = 20;
/// Issue #1146: once the hot vouch-history window reaches this length, the
/// oldest entries are cut over into a single `ArchivedVouchHistory` batch,
/// bringing the hot window back down to `MAX_HOT_VOUCH_HISTORY_ENTRIES`.
pub const VOUCH_HISTORY_ARCHIVE_TRIGGER_ENTRIES: u32 = 30;
/// Issue #1179: target size of the "hot" per-(borrower, voucher, token)
/// vouch audit-trail window kept after an archival cutover. Mirrors the
/// `MAX_HOT_VOUCH_HISTORY_ENTRIES` bounding strategy used for `VouchHistory`.
pub const MAX_HOT_VOUCH_AUDIT_TRAIL_ENTRIES: u32 = 20;
/// Issue #1179: once the hot vouch audit-trail window reaches this length,
/// the oldest entries are cut over into a single `ArchivedVouchAuditTrail`
/// batch, bringing the hot window back down to `MAX_HOT_VOUCH_AUDIT_TRAIL_ENTRIES`.
pub const VOUCH_AUDIT_TRAIL_ARCHIVE_TRIGGER_ENTRIES: u32 = 30;
/// Issue #1146: maximum number of items returned by a single page of any
/// `*_page` read function, regardless of the caller-requested `limit`.
pub const MAX_PAGE_SIZE: u32 = 50;
// ── Issue #1285: Soroban Persistent Storage TTL Constants ─────────────────────
//
// Soroban persistent entries have a TTL measured in ledgers (1 ledger ≈ 5 s).
// Once the TTL lapses the entry is archived off the live ledger; any subsequent
// read will trap unless the entry is restored first.  We therefore extend_ttl
// on every hot write path so long-lived entries never silently disappear.
//
// Ledger rate: ~17_280 ledgers / day (5 s / ledger).
//
/// extend_ttl threshold for loan/vouch/queue entries (~30 days in ledgers).
/// If the current TTL is already above this we skip the extend to save CPU.
pub const PERSISTENT_TTL_THRESHOLD_LEDGERS: u32 = 30 * 17_280; // 518_400
/// Target TTL for loan/vouch/queue entries after extension (~1 year in ledgers).
pub const PERSISTENT_TTL_TARGET_LEDGERS: u32 = 365 * 17_280; // 6_307_200
/// Target TTL for the instance storage (config/admins/paused/etc.) (~1 year).
pub const INSTANCE_TTL_TARGET_LEDGERS: u32 = 365 * 17_280; // 6_307_200
/// Threshold for instance TTL bumps (~30 days).
pub const INSTANCE_TTL_THRESHOLD_LEDGERS: u32 = 30 * 17_280; // 518_400

/// Default governance voting period for slash-threshold proposals, in seconds (7 days).
pub const DEFAULT_VOTING_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60;
/// Minimum delay before a timelocked governance action may be executed, in seconds (24 hours).
pub const TIMELOCK_DELAY: u64 = 24 * 60 * 60;
/// Default timelock delay before a designated successor admin may claim admin rights, in seconds (24 hours).
pub const SUCCESSOR_CLAIM_TIMELOCK_SECS: u64 = 24 * 60 * 60;
/// Maximum window after `eta` within which a timelocked action must be executed, in seconds (72 hours).
pub const TIMELOCK_EXPIRY: u64 = 72 * 60 * 60;
/// Cross-chain vote attestations older than this (relative to the ledger clock) are rejected as stale (10 minutes).
pub const VOTE_ATTESTATION_MAX_AGE_SECS: u64 = 10 * 60;
/// Cross-chain vote attestations timestamped further than this into the future are rejected, in seconds (60).
pub const VOTE_ATTESTATION_MAX_SKEW_SECS: u64 = 60;
/// Minimum lock period for a vouch before it can be withdrawn, in seconds (7 days).
/// Protects against flash-loan-style attacks where an attacker stakes, borrows, then
/// immediately withdraws.
pub const MIN_VOUCH_LOCK_PERIOD: u64 = 7 * 24 * 60 * 60;

/// Maximum reputation bonus for vouchers, in basis points (100 = 1%).
pub const REPUTATION_BONUS_MAX_BPS: i128 = 100;

/// Duration of slash escrow period before funds are burned or returned, in seconds (7 days).
pub const SLASH_APPEAL_PERIOD: u64 = 7 * 24 * 60 * 60;

/// Quorum required to overturn a slash appeal, in basis points (6667 = 2/3).
pub const APPEAL_OVERRIDE_QUORUM_BPS: u32 = 6_667;

/// Fraction of slashed funds routed to the insurance pool (2000 = 20%).
pub const SLASH_TO_INSURANCE_BPS: u32 = 2_000;
/// Default insurance fee on loan disbursement (50 = 0.5%).
pub const DEFAULT_INSURANCE_FEE_BPS: u32 = 50;
/// Default max insurance payout as % of slashed stake (2500 = 25%).
pub const DEFAULT_INSURANCE_COVERAGE_BPS: u32 = 2_500;

/// Extension fee charged when a borrower requests a loan extension, in basis points (100 = 1%).
pub const EXTENSION_FEE_BPS: i128 = 100;

/// Maximum number of extensions allowed per loan.
pub const MAX_EXTENSIONS_PER_LOAN: u32 = 2;

/// Issue #10: Default maximum number of consecutive refinances in a loan chain.
/// A value of 3 means a borrower can refinance up to 3 times before needing to
/// close the chain and start fresh.
pub const DEFAULT_MAX_REFINANCES_PER_LOAN_CHAIN: u32 = 3;

/// Issue #10: Default minimum cooldown between consecutive refinances, in seconds.
/// 7 days prevents rapid chaining that could abuse prepayment-penalty timing.
pub const DEFAULT_REFINANCE_COOLDOWN_SECS: u64 = 7 * 24 * 60 * 60;

/// Default liquidity mining reward rate in basis points per epoch (50 = 0.5% per 7 days).
pub const DEFAULT_LIQUIDITY_MINING_RATE_BPS: u32 = 50;

/// Issue #1238: Precision scalar used in yield-per-token accounting (10^12).
/// Yield-per-token is stored multiplied by this factor to preserve sub-stroop precision.
pub const YIELD_PER_TOKEN_PRECISION: i128 = 1_000_000_000_000;

/// Issue #1238: Default staking pool yield rate in basis points per year (500 = 5% APY).
pub const DEFAULT_STAKING_POOL_APY_BPS: u32 = 500;

/// Issue #1238: Minimum unstake queue delay in seconds (24 hours).
pub const STAKING_UNSTAKE_DELAY_SECS: u64 = 24 * 60 * 60;

/// Default dynamic slash threshold setting (false = disabled by default).
pub const DEFAULT_DYNAMIC_SLASH_THRESHOLD: bool = false;

/// Default loan-size-based slash scaling setting (false = disabled by default).
pub const DEFAULT_LOAN_SIZE_SLASH_ENABLED: bool = false;

/// Default maximum slash rate for the largest loans, in basis points (8000 = 80%).
/// When loan-size scaling is enabled, slash_bps is the floor (small loans) and
/// this is the ceiling (loans at or above the total staked collateral).
pub const DEFAULT_LOAN_SIZE_SLASH_MAX_BPS: i128 = 8_000;

/// Default borrower repayment confirmation requirement (false = disabled by default).
pub const DEFAULT_CONFIRMATION_REQUIRED: bool = false;

/// Default quorum for voucher-based slash votes, in basis points (6667 ≈ 66.67%).
/// Requires approximately 2/3 of total vouched stake to approve before a slash executes.
pub const DEFAULT_SLASH_VOTE_QUORUM_BPS: u32 = 6_667;

/// Minimum elapsed time between successive slash proposals for the same borrower, in seconds
/// (7 days). Prevents spam proposals and gives borrowers time to resolve issues between rounds.
pub const DEFAULT_SLASH_PROPOSAL_COOLDOWN_SECS: u64 = 7 * 24 * 60 * 60;

/// Default rate limit: 10 calls per window.
pub const DEFAULT_RATE_LIMIT_COUNT: u32 = 10;
/// Default rate limit window: 60 seconds.
pub const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Timelock delay for decrease_stake during an active loan, in seconds (7 days).
pub const DECREASE_STAKE_TIMELOCK: u64 = 7 * 24 * 60 * 60;

/// Withdrawal request timelock delay, in seconds (24 hours).
pub const WITHDRAWAL_TIMELOCK_DELAY: u64 = 24 * 60 * 60;

/// Maximum number of deferment periods allowed per loan.
pub const MAX_DEFERMENT_PERIODS: u32 = 3;

/// Duration of each deferment period, in seconds (30 days).
pub const DEFERMENT_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// Penalty applied to partial mid-loan withdrawals, in basis points (1000 = 10%).
pub const PARTIAL_WITHDRAWAL_PENALTY_BPS: i128 = 1_000;

/// Default reputation score decay per month in basis points (100 = 1% per month).
/// Encourages active participation and prevents stale scores from granting perpetual benefits.
pub const DEFAULT_REPUTATION_SCORE_DECAY_BPS: u32 = 100;

/// Yield stream period in seconds (7 days).
pub const YIELD_STREAM_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

/// Maximum priority fee as a percentage of voucher stake, in basis points (1000 = 10%).
/// Prevents uncapped front-running by capping the priority fee to a fraction of the voucher's own stake.
pub const MAX_PRIORITY_FEE_BPS: i128 = 1_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserRole {
    Admin,
    User,
    Guest,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitTier {
    pub role: UserRole,
    pub max_requests_per_hour: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfig {
    pub window_secs: u64,
    pub max_calls: u32,
    pub enabled: bool,
    pub tiers: Vec<RateLimitTier>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminRole {
    SuperAdmin,
    Treasurer,
    Monitor,
    Slasher,
    GovernanceOperator,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminPermission {
    Slash,
    Pause,
    UpdateConfig,
    ManageFees,
    ReadAnalytics,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionMatrix {
    pub role: AdminRole,
    pub permissions: Vec<AdminPermission>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Voucher,
    Borrower,
    Governance,
    Oracle,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RolePermissions {
    pub role: Role,
    pub can_vouch: bool,
    pub can_request_loan: bool,
    pub can_repay: bool,
    pub can_slash: bool,
    pub can_gov: bool,
}

/// Maximum fraction of stake that can be partially withdrawn during an active loan (50%).
pub const PARTIAL_WITHDRAWAL_MAX_BPS: i128 = 5_000;

/// Minimum slash threshold when protocol health is excellent, in basis points (2500 = 25%).
pub const MIN_DYNAMIC_SLASH_BPS: i128 = 2_500;

/// Maximum slash threshold when protocol health is poor, in basis points (7500 = 75%).
pub const MAX_DYNAMIC_SLASH_BPS: i128 = 7_500;

/// Health threshold below which slash penalty increases, in basis points (8000 = 80%).
pub const HEALTH_THRESHOLD_BPS: i128 = 8_000;

/// Default slash delay period to allow for disputes, in seconds (7 days).
pub const DEFAULT_SLASH_DELAY_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Duration of one reporting month, in seconds (30 days).
pub const MONTHLY_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// Default premium rate for slashing insurance opt-in, in basis points (100 = 1%).
pub const DEFAULT_INSURANCE_PREMIUM_BPS: u32 = 100;

// ── Loan Extension ────────────────────────────────────────────────────────────

/// A pending loan extension request. Created by the borrower; approved by vouchers.
#[contracttype]
#[derive(Clone)]
pub struct LoanExtensionRequest {
    /// The borrower requesting the extension.
    pub borrower: Address,
    /// Loan ID being extended.
    pub loan_id: u64,
    /// Requested additional duration in seconds.
    pub extension_secs: u64,
    /// Timestamp when the request was created.
    pub requested_at: u64,
    /// Vouchers who have approved this extension.
    pub approvals: Vec<Address>,
    /// Extension fee paid (in stroops), deducted from borrower on approval.
    pub fee_paid: i128,
    /// How many times this loan has already been extended.
    pub extension_count: u32,
}
/// Slash escrow period before funds are permanently burned, in seconds (30 days).
pub const SLASH_ESCROW_PERIOD: u64 = 30 * 24 * 60 * 60;

// ── Escrow Status ─────────────────────────────────────────────────────────────

/// Status of a repayment held in oracle-verified escrow (#666/#667).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// No escrow — repayment released immediately (default).
    None,
    /// Repayment held pending oracle verification.
    Pending,
    /// Oracle approved — funds released to vouchers.
    Released,
    /// Oracle rejected — funds returned to borrower.
    Rejected,
}

// ── Daily-Compound Interest Constants ─────────────────────────────────────────
//
// Daily compounding is layered *on top of* the static yield locked in at
// disbursement (`total_yield`).  It is computed each time `repay()` is called
// using the outstanding principal and the number of whole days elapsed since
// `last_interest_calc`.
//
// Formula (integer, truncating):
//   daily_interest = outstanding_principal * COMPOUND_RATE_BPS / 10_000 / 365
//   (applied once per elapsed day)
//
// The approximation 1/365 avoids floating-point.  For a 365-day loan at 5 bps
// daily the accrued interest is small relative to principal, which is the
// intended behaviour for a microlending platform.

/// Seconds in one day (86 400).
pub const SECS_PER_DAY: u64 = 24 * 60 * 60;

/// Annual interest rate in basis points used for daily compounding accrual
/// (default 500 bps = 5% per year → ≈0.0137 bps/day).
pub const COMPOUND_RATE_BPS: i128 = 500;

// ── Milestone Bonus Constants ─────────────────────────────────────────────────
//
// When a borrower has repaid a certain fraction of their total obligation
// (principal + static yield), they earn a one-time discount applied as a
// reduction in accrued compound interest.  Each milestone may only fire once
// per loan.  Milestones are expressed as thresholds in per-mille (‰) of the
// total obligation that has been repaid, and as a discount in basis points
// applied to the *remaining accrued_interest*.

/// Borrower has repaid ≥ 25% of the total obligation.
/// Reward: 10% reduction of the remaining accrued compound interest.
pub const MILESTONE_25_PCT_PERMILLE: u32 = 250;
pub const MILESTONE_25_DISCOUNT_BPS: i128 = 1_000; // 10% off accrued interest

/// Borrower has repaid ≥ 50% of the total obligation.
/// Reward: 20% reduction of the remaining accrued compound interest.
pub const MILESTONE_50_PCT_PERMILLE: u32 = 500;
pub const MILESTONE_50_DISCOUNT_BPS: i128 = 2_000; // 20% off accrued interest

/// Borrower has repaid ≥ 75% of the total obligation.
/// Reward: 30% reduction of the remaining accrued compound interest.
pub const MILESTONE_75_PCT_PERMILLE: u32 = 750;
pub const MILESTONE_75_DISCOUNT_BPS: i128 = 3_000; // 30% off accrued interest

/// Bitmask flags stored in `milestone_bonus_applied`.
/// Each bit represents one milestone that has already fired.
pub const MILESTONE_FLAG_25: u32 = 0b001;
pub const MILESTONE_FLAG_50: u32 = 0b010;
pub const MILESTONE_FLAG_75: u32 = 0b100;

// ── Loan Status ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    None,
    Active,
    Repaid,
    /// #663: Borrower repaid some but less than partial_default_threshold_bps of total owed.
    PartialDefault,
    Defaulted,
    /// #664: Default was forgiven by admin.
    ForgivenDefault,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanStatusEx {
    None,
    Active,
    Suspended,
    Repaid,
    PartialDefault,
    Defaulted,
    ForgivenDefault,
}

impl From<LoanStatus> for LoanStatusEx {
    fn from(status: LoanStatus) -> Self {
        match status {
            LoanStatus::None => LoanStatusEx::None,
            LoanStatus::Active => LoanStatusEx::Active,
            LoanStatus::Repaid => LoanStatusEx::Repaid,
            LoanStatus::PartialDefault => LoanStatusEx::PartialDefault,
            LoanStatus::Defaulted => LoanStatusEx::Defaulted,
            LoanStatus::ForgivenDefault => LoanStatusEx::ForgivenDefault,
        }
    }
}

/// Interest rate type for a loan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    /// Fixed rate locked at disbursement (yield_bps from Config).
    Fixed,
    /// Variable rate tied to an external index; recalculated on each repayment.
    Variable,
}

// ── Pause State Machine ───────────────────────────────────────────────────────

/// Contract pause state for the Normal → Paused → Thawing → Normal state machine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PauseMode {
    /// Contract is operating normally.
    None,
    /// Contract is fully paused — all writes are blocked.
    Paused,
    /// Contract is thawing — only reads and withdrawals are allowed.
    /// Automatically transitions to `None` after `thaw_duration` seconds.
    Thawing,
}

/// Timestamps recorded when the contract enters or exits a thaw period.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThawState {
    /// Ledger timestamp when `pause()` was called.
    pub pause_timestamp: u64,
    /// Duration of the thaw window in seconds (default 24 h = 86_400).
    pub thaw_duration: u64,
    /// Ledger timestamp when `begin_thaw()` was called.
    pub thaw_start_timestamp: u64,
}

/// Duration of the thaw period in seconds (24 hours).
pub const THAW_DURATION_SECS: u64 = 24 * 60 * 60;

// ── Governance Proposal Status ─────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    /// Proposal is under active voting.
    Active,
    /// Proposal has passed and is executable.
    Passed,
    /// Proposal has been rejected.
    Rejected,
    /// Proposal voting period has expired.
    Expired,
    /// Proposal has been executed.
    Executed,
}

// ── Sybil Resistance ─────────────────────────────────────────────────────────

/// Estimated cost (in stroops) for an attacker to Sybil-attack a borrower's
/// voucher configuration and achieve the same credit-score / governance weight.
///
/// Returned by `estimate_sybil_attack_cost` in `vouch.rs`.
#[contracttype]
#[derive(Clone)]
pub struct SybilAttackCostEstimate {
    /// Minimum aggregate stake (in stroops) an attacker must commit to match
    /// the total reputation-weighted stake of the real voucher set.
    pub min_stake_stroops: i128,
    /// Minimum time (in seconds) the attacker must hold that stake before the
    /// vouches become age-eligible for reputation credit.
    pub min_lock_secs: u64,
    /// Number of vouches in the legitimate set (for reference).
    pub voucher_count: u32,
    /// Total reputation-weighted stake of the current legitimate set.
    pub total_weighted_stake: i128,
    /// Ledger timestamp when this estimate was computed.
    pub computed_at: u64,
}

// ── Issue #1074-1077: Multi-Token and Bridge Support ────────────────────────

/// Metadata for a bridged (non-Stellar) token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenBridgeMetadata {
    /// Address of the bridge contract managing this token.
    pub bridge_address: Address,
    /// Source chain ID where this token originated.
    pub source_chain_id: u32,
    /// Source token address on the origin chain (may be a different format).
    pub source_token_address: String,
    /// Human-readable name of the bridged token.
    pub name: String,
    /// Number of decimal places for this token.
    pub decimals: u32,
    /// Whether this bridge is currently active.
    pub active: bool,
}

/// Configuration for token swaps during repayment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenSwapConfig {
    /// DEX contract address to use for swaps.
    pub dex_address: Address,
    /// Slippage tolerance in basis points (e.g. 300 = 3%).
    pub slippage_tolerance_bps: u32,
    /// Whether token swaps are enabled.
    pub enabled: bool,
    /// Minimum swap amount to avoid dust (in stroops).
    pub min_swap_amount: i128,
}

/// Liquidity tier for dynamic yield bonuses.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityTier {
    /// Tier index (0-3): 0=most liquid, 3=illiquid.
    pub tier: u32,
    /// Yield bonus in basis points for this tier.
    pub bonus_bps: i128,
}

/// Default liquidity tier bonuses (in basis points).
pub const DEFAULT_LIQUIDITY_TIER_BONUSES: [i128; 4] = [0, 50, 150, 300];

/// Audit event type for vouch history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VouchAuditEventType {
    Created,
    Modified,
    Withdrawn,
    Slashed,
    Restored,
}

/// A single audit event in the vouch history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VouchAuditEvent {
    /// Type of audit event.
    pub event_type: VouchAuditEventType,
    /// Timestamp when the event occurred.
    pub timestamp: u64,
    /// Amount involved (stake change, slash amount, etc.).
    pub amount: i128,
    /// Additional details as free text.
    pub details: String,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Loan(u64),                   // loan_id → LoanRecord
    ActiveLoan(Address),         // borrower → active loan_id
    LatestLoan(Address),         // borrower → latest loan_id
    Vouches(Address),            // borrower → Vec<VouchRecord>
    VoucherHistory(Address),     // voucher → Vec<Address> (borrowers backed)
    Config,                      // Config struct: all configurable protocol parameters
    Deployer,                    // Address that deployed the contract; guards initialize
    SlashTreasury,               // i128 accumulated slashed funds
    Paused,                      // bool: true when contract is paused
    PauseMode,                   // PauseMode enum: None, Paused, or Thawing
    ThawState,                   // ThawState: pause and thaw timestamps
    BorrowerList,                // Vec<Address> of all borrowers who have ever requested a loan
    ReputationNft,               // Address of the ReputationNftContract
    MinStake,                    // i128 minimum stake amount per vouch
    MaxLoanAmount,               // i128 maximum individual loan size (0 = no cap)
    MinVouchers,     // u32 minimum number of distinct vouchers required (0 = no minimum)
    LoanCounter,     // u64: monotonically increasing loan ID counter
    LoanPool(u64),   // pool_id → LoanPoolRecord
    LoanPoolCounter, // u64: monotonically increasing pool ID counter
    PendingAdmin,    // Address of the pending admin (two-step transfer)
    /// Issue #1443: Earliest timestamp when the designated successor admin may claim admin rights
    SuccessorAdminClaimableAt,
    RepaymentCount(Address), // borrower → u32 total successful repayments
    LoanCount(Address), // borrower → u32 total historical loans disbursed
    DefaultCount(Address), // borrower → u32 total defaults (slash + auto_slash + claim_expired)
    /// Issue #1371: protocol-wide total default count, incremented alongside every
    /// per-borrower `DefaultCount` update. Feeds `circuit_breaker::get_current_default_rate`.
    TotalDefaultCount,
    ProtocolFeeBps,  // u32: protocol fee in basis points
    FeeTreasury,     // Address: recipient of collected protocol fees
    LastVouchTimestamp(Address), // voucher → u64 last vouch timestamp
    VouchCooldownSecs, // u64 cooldown between vouch calls (default 24 hours)
    Timelock(u64),   // proposal_id → TimelockProposal
    TimelockCounter, // u64 monotonically increasing proposal ID
    Blacklisted(Address), // borrower → bool permanently banned
    BlacklistReason(Address), // borrower → Bytes reason for blacklisting (Issue #1073)
    VoucherWhitelist(Address), // voucher → bool allowed to vouch
    WhitelistEnabled, // bool: true when voucher whitelist is enabled (opt-in)
    ExtensionConsents(Address), // borrower → Vec<Address> vouchers who consented to extension
    SlashVote(Address), // borrower → SlashVoteRecord
    SlashVoteQuorum, // u32 quorum in basis points (e.g. 5000 = 50%)
    ReferredBy(Address), // borrower → Address of referrer
    ReferralBonusBps, // u32 referral bonus in basis points (default 100 = 1%)
    MaxVouchersPerBorrower, // u32 maximum number of vouchers per borrower (default 50)
    BorrowerCollateral(Address), // borrower → i128 collateral amount deposited
    BorrowerCollateralToken(Address), // borrower → Address token used for collateral
    InsurancePool,           // i128 total funds contributed to the insurance pool
    InsuranceClaim(u64),     // loan_id → bool: has any claim been made (legacy single-claim guard)
    InsuranceFeeBps,         // u32: protocol fee routed to insurance pool per loan (default 50 = 0.5%)
    InsuranceCoverageBps,    // u32: max payout as % of slashed stake (default 2500 = 25%)
    InsuranceVoucherClaim(u64, Address), // (loan_id, voucher) → i128 amount already claimed
    VouchHistory(Address, Address, Address), // (borrower, voucher, token) → Vec<VouchHistoryEntry>
    VouchDelegation(Address, Address, Address), // (borrower, original_voucher, token) → Address (delegate)
    /// Issue #1069: Vote delegation - voucher → delegate address for governance votes
    VoteDelegation(Address),
    PendingSlashExecution(Address), // borrower → PendingSlashRecord
    YieldReserve,            // i128 balance of the yield reserve
    SlashEscrow(Address),    // borrower → (i128 amount, u64 release_timestamp)
    SlashAudit(Address),     // borrower → SlashRecord (latest slash for borrower)
    SlashRecord(u64),        // slash_id → SlashRecord
    SlashRecordCounter,      // u64 monotonic slash ID counter
    BorrowerRegistered(Address), // borrower → registration timestamp
    // Issue #598-601 additions
    PrepaymentPenaltyBps,    // u32: prepayment penalty in basis points
    YieldDistribution(u64),  // loan_id → Vec<YieldDistributionEntry>
    AdminAction(u64),        // action_id → AdminActionProposal
    AdminActionCounter,      // u64: monotonically increasing admin action ID
    SlashAppeal(Address, Address), // (borrower, voucher) → SlashAppealRecord (Issue #552)
    SlashEscrowAppeal(Address), // borrower → SlashAppealRecord (Issue #841: escrow-based appeal)
    /// Mutual-exclusion flag: a #552-style evidence appeal is in progress for this borrower.
    /// Set true by `appeal_slash_with_evidence`, cleared by `execute_slash_appeal` or
    /// `vote_on_slash_appeal` (when approve=false). Prevents the #841 escrow path from
    /// running concurrently and potentially double-refunding the same voucher. (Issue #1450)
    EvidenceAppealPending(Address), // borrower → bool
    /// Slash-threshold governance proposal id → proposal record.
    SlashThresholdProposal(u64),
    SlashThresholdProposalCounter,
    /// Per-borrower timestamp of the last successful slash.
    LastSlashedAt(Address),
    /// Per-borrower timestamp of the most recent slash proposal initiation.
    /// Used to enforce the 7-day cooldown between successive slash proposals.
    LastSlashProposalAt(Address),
    /// Refinance record for a loan: loan_id → RefinanceRecord
    RefinanceRecord(u64),
    /// Borrower repayment confirmation for oracle-gated repayment: loan_id → bool
    RepaymentConfirmation(u64),
    /// Cached total weighted stake per borrower per token: (borrower, token) → i128
    /// Used for O(1) eligibility checks; invalidated on vouch operations.
    TotalWeightedStakeCache(Address, Address),
    /// Archived loan records: archive_id → ArchivedLoanRecord
    /// Old completed or slashed loans are moved here to reduce persistent storage.
    ArchivedLoan(u64),
    /// Archive counter for generating unique archive IDs
    ArchiveCounter,
    /// Archived vouch history: (borrower, voucher, token, batch_id) → Vec<VouchHistoryEntry>
    /// Old vouch history entries are moved here when history grows beyond a threshold.
    ArchivedVouchHistory(Address, Address, Address, u32),
    /// IPFS archive reference for loans: archive_id → IpfsArchiveReference
    /// Maps archive IDs to their IPFS content hashes for off-chain storage.
    IpfsLoanArchive(u64),
    /// IPFS archive reference for vouch history: archive_id → IpfsArchiveReference
    IpfsVouchHistoryArchive(u64),
    /// Counter for IPFS archives created
    IpfsArchiveCounter,
    /// Flag indicating if an archive has been backed up to IPFS: archive_id → bool
    IpfsBackedArchive(u64),
    /// Admin config-update proposal id → proposal record.
    ConfigUpdateProposal(u64),
    ConfigUpdateProposalCounter,
    /// Issue #599/#600: (voucher, borrower) → WithdrawalRequest (pending timelock withdrawal)
    PendingWithdrawal(Address, Address),
    /// Confidential vouch commitment: (voucher, borrower) → commitment record
    VouchCommitment(Address, Address),
    /// Confidential loan commitment: borrower → commitment record
    LoanCommitment(Address),
    /// Whether a confidential vouch commitment has already been revealed/settled: (voucher, borrower) → bool
    VouchCommitmentRevealed(Address, Address),
    /// Whether a confidential loan commitment has already been revealed/settled: borrower → bool
    LoanCommitmentRevealed(Address),
    /// Monotonic counter for confidential proof records
    ZkProofCounter,
    /// Confidential proof record by ID
    ZkProofRecord(u64),
    /// Issue #601: borrower → LoanExtensionRequest
    LoanExtension(Address),
    /// Issue #598: loan_id → Vec<PaymentRecord> (payment history)
    PaymentHistory(u64),
    /// Voucher cumulative reputation stats: voucher → VoucherStats
    VoucherStats(Address),
    /// Withdrawal queue: borrower → Vec<QueuedWithdrawal>
    WithdrawalQueue(Address),
    // #634: Liquidity Mining
    LastMiningClaim(Address),
    // #635: Vouch Snapshot for Governance
    VouchSnapshot(u32),
    // #636: Staking Derivatives
    StakingDerivative(Address, Address),
    // #637: Fraud Detection
    VoucherFraudScore(Address),
    /// Issue #637: on-demand fraud detection configuration
    FraudScoreConfig,
    /// Repayment dispute raised by a voucher: (borrower, voucher) -> DisputeRecord
    RepaymentDispute(Address, Address),
    // #667: Oracle address for repayment verification
    OracleAddress,
    // #667: External credit score per borrower
    ExternalCreditScore(Address),
    // #666: Escrowed repayment amount per borrower (held pending oracle verification)
    EscrowAmount(Address),
    /// Monthly slashing transparency report: month_id → SlashingReportRecord.
    /// month_id = unix_timestamp / MONTHLY_PERIOD_SECS
    SlashingReport(u64),
    /// Issue #1444: Per-month index of slash record IDs: month_id → Vec<u64>
    SlashesByMonth(u64),
    /// Per-vouch insurance opt-in: (voucher, borrower) → bool (insured).
    VoucherInsurance(Address, Address),
    /// Cross-chain bridge validation status: (voucher, chain_id) → bool.
    BridgeValidated(Address, u32),
    /// Registered cross-chain bridges: Vec<BridgeRecord>
    Bridges,
    /// origin_chain → the Ed25519 public key trusted to sign attestations from it.
    BridgePublicKey(u32),
    /// (origin_chain, nonce) → true once an attestation with that nonce has been consumed.
    BridgeNonceUsed(u32, u64),
    /// (origin_chain, loan_id) → the mirrored CrossChainLoanMetadata for that origin loan.
    MirroredLoan(u32, u64),
    /// borrower → the latest cross-chain reputation snapshot mirrored in for them.
    CrossChainReputation(Address),
    /// Issue #687: admin removal proposal id → AdminRemovalProposal
    AdminRemovalProposal(u64),
    /// Issue #687: monotonically increasing admin removal proposal counter
    AdminRemovalProposalCounter,
    /// Issue #686: accumulated admin compensation pool balance (i128 stroops)
    AdminCompensation,
    /// Issue #686: last compensation claim timestamp per admin address
    AdminLastClaim(Address),
    RolePermissions(Address), // address -> RolePermissions
    RateLimit(Address),        // address -> (u64 last_call_window_start, u32 call_count)
    /// Issue #16: admin address -> AdminRole
    AdminRole(Address),
    /// Issue #742: current semantic contract version
    ContractVersion,
    /// Issue #742: version history entries by index
    ContractVersionHistory(u32),
    /// Issue #742: number of version history entries
    ContractVersionHistoryCount,
    /// Issue #743: deployment record by index
    DeploymentRecord(u32),
    /// Issue #743: total number of deployment records
    DeploymentRecordCount,
    /// Issue #744: rollback snapshot of config keyed by version index
    RollbackSnapshot(u32),
    /// Governance proposal id → GovernanceProposal
    GovernanceProposal(u64),
    /// Governance proposal counter (monotonically increasing)
    GovernanceProposalCounter,
    /// Governance queue configuration
    GovernanceQueueConfig,
    /// Credit score record for a borrower
    CreditScore(Address),
    /// Credit score configuration
    CreditScoreConfig,
    /// Loan syndication record
    LoanSyndication(u64),
    /// Syndication counter (monotonically increasing)
    SyndicationCounter,
    /// Syndication configuration
    SyndicationConfig,
    /// Syndication member index (syndication_id, member_address) → SyndicationMember
    SyndicationMember(u64, Address),
    /// Syndication repayment records
    SyndicationRepayment(u64, u64), // syndication_id, repayment_index
    /// Syndication repayment counter
    SyndicationRepaymentCounter(u64), // syndication_id → counter
    /// Reputation NFT badge for excellent credit tier: borrower → ReputationNFTRecord
    ReputationNFTBadge(Address),
    // ── Issue #863: Vouch Cooldown Bypass ────────────────────────────────────
    /// Per-voucher emergency bypass flag: voucher → bool
    EmergencyCooldownBypass(Address),
    /// Cooldown bypass request: (borrower, voucher) → CooldownBypassRequest
    CooldownBypass(Address, Address),
    // ── Issue #867: Cross-Collateral Vouch Pools ─────────────────────────────
    CollateralPool(u64),
    CollateralPoolCounter,
    BorrowerPool(Address, u64),
    // ── Issue #868: Gradual Unstaking ─────────────────────────────────────────
    GradualUnstake(Address, Address),
    // ── Issue #882: Loan Insurance Integration ───────────────────────────────
    /// loan_id → bool: whether insurance was collected at disbursement
    InsuranceLinked(u64),
    // ── Issue #884: Prepayment Bonus ─────────────────────────────────────────
    /// Configurable prepayment bonus rate in basis points
    PrepaymentBonusBps,
    // ── Issue #885: Loan Status Privacy ──────────────────────────────────────
    /// borrower → LoanPrivacyLevel
    LoanPrivacy(Address),
    // ── Issue #887: Loan Subordination and Cascading Debt Hierarchy ──────────
    /// (senior_loan_id, subordinate_loan_id) → SubordinationRecord
    SubordinationRelation(u64, u64),
    /// senior_loan_id → Vec<u64> (IDs of all subordinate loans ordered by priority)
    SubordinateLoansList(u64),
    /// subordinate_loan_id → u64 (ID of direct senior loan, if any)
    SeniorLoanOf(u64),
    /// senior_loan_id → CascadingDefault (tracks cascade triggered by default)
    CascadingDefaultRecord(u64),
    /// Waterfall distribution configuration for a borrower
    WaterfallConfig(Address),
    // ── Issue #934: Yield Calculation Caching ──────────────────────────────────
    /// (borrower, voucher) → CachedYieldRecord
    YieldCache(Address, Address),
    // ── Cache infrastructure (Issue #724) ──────────────────────────────────────
    /// LRU index counter for cache eviction
    LruIndex,
    /// Oldest loan cache entry ID for LRU eviction
    LruOldestLoanId,
    // ── Reentrancy Guard ──────────────────────────────────────────────────────
    /// bool: true when a state-mutating operation is in progress
    Locked,
    // ── Nonce tracking (Issue #64) ────────────────────────────────────────────
    /// Address → u64: last consumed nonce for replay protection
    Nonce(Address),
    // ── Oracle price (Issue #64) ──────────────────────────────────────────────
    /// Symbol → OraclePriceRecord
    OraclePrice(soroban_sdk::Symbol),
    // ── Graduated threat level (Issue #65) ────────────────────────────────────
    /// ThreatLevel enum value
    ThreatLevelKey,
    // ── Multi-tier admin thresholds (Issue #893) ──────────────────────────────
    /// MultiTierAdminThresholds configuration (stored in instance storage)
    MultiTierAdminThresholds,
    // ── Risk threshold governance (Issue #903) ────────────────────────────────
    /// proposal_id → RiskThresholdProposal
    RiskThresholdProposal(u64),
    /// monotonically increasing risk threshold proposal counter
    RiskThresholdCounter,
    /// (proposal_id, voter) → bool (has voted)
    RiskThresholdVote(u64, Address),
    // ── Fee structure governance (Issue #904) ─────────────────────────────────
    /// proposal_id → FeeStructureProposal
    FeeStructureProposal(u64),
    /// monotonically increasing fee structure proposal counter
    FeeStructureCounter,
    /// (proposal_id, voter) → bool (has voted)
    FeeStructureVote(u64, Address),
    // ── Withdrawal timelock (Issue #905) ──────────────────────────────────────
    /// (borrower, voucher) → withdrawal timelock record
    WithdrawalTimelock(u64),
    /// monotonically increasing withdrawal timelock counter
    WithdrawalTimelockCounter,
    // ── Cross-chain proposal sync (Issue #906) ─────────────────────────────────
    /// proposal_id → CrossChainProposalSync
    CrossChainProposalSync(u64),
    /// monotonically increasing cross-chain proposal sync counter
    CrossChainSyncCounter,
    // ── Yield stream (Issue #907) ──────────────────────────────────────────────
    /// loan_id → YieldStreamState
    YieldStreamState(u64),
    /// (loan_id, voucher) → VoucherYieldClaim
    VoucherYieldClaim(u64, Address),
    // ── Periodic payments (Issue #908) ────────────────────────────────────────
    /// loan_id → PeriodicPaymentConfig
    PeriodicPaymentConfig(u64),
    /// loan_id → PeriodicPaymentStatus
    PeriodicPaymentStatus(u64),
    // ── Vouch groups (Issue #909) ──────────────────────────────────────────────
    /// group_id → VouchGroup
    VouchGroup(u64),
    /// monotonically increasing vouch group counter
    VouchGroupCounter,
    /// voucher → Vec<u64> group IDs the voucher belongs to
    VoucherGroupIds(Address),
    // ── Vouch merkle root (Issue #910) ────────────────────────────────────────
    /// borrower → BytesN<32> merkle root of vouch set
    VouchMerkleRoot(Address),
    // ── Batch transfers (Issue #935) ──────────────────────────────────────────
    /// Vec<BatchTransfer> pending transfer queue
    PendingTransfers,
    // ── Lazy slash queue (Issue #937) ─────────────────────────────────────────
    /// Vec<LazySlashEntry> queued slash operations
    LazySlashQueue,
    // ── Custom attributes ────────────────────────────────────────────────────
    /// Address → Vec<AttributeEntry>
    CustomAttributes(Address),
    // ── Forbearance (Issue #878) ──────────────────────────────────────────────
    /// loan_id → ForbearanceRecord
    Forbearance(u64),
    // ── Dynamic rate (Issue #881) ──────────────────────────────────────────────
    /// DynamicRateConfig (global config)
    DynamicRateConfig,
    /// borrower → BorrowerDynamicRate
    BorrowerDynamicRate(Address),
    // ── API versioning ─────────────────────────────────────────────────────────
    /// Current API version string
    ApiVersion,
    // ── Emergency admin revocation ─────────────────────────────────────────────
    /// Emergency admin revocation record — Address → bool (true = revoked).
    /// Revoked admins are excluded from admin approval checks.
    RevokedAdmin(Address),
    // ── Sybil resistance (Issue #sybil) ─────────────────────────────────────
    /// borrower → SybilAttackCostEstimate
    /// Cached estimate of the cost (in stroops) to Sybil-attack a borrower's
    /// voucher configuration. Invalidated when vouches change.
    SybilAttackCost(Address),
    /// Issue #1146: (borrower, voucher, token) → u32 number of archive batches
    /// created so far for this relationship's vouch history. The index needed
    /// to enumerate `ArchivedVouchHistory` batches in order (0..count).
    VouchHistoryArchiveCount(Address, Address, Address),
    // ── Vouch audit trail (Issue #1179) ──────────────────────────────────────
    /// (borrower, voucher, token) → Vec<VouchAuditEvent>: bounded "hot" window
    /// of audit events (created / stake increased / stake decreased /
    /// withdrawn) for this vouch relationship.
    VouchAuditTrail(Address, Address, Address),
    /// Archived vouch audit trail: (borrower, voucher, token, batch_id) →
    /// Vec<VouchAuditEvent>. Old audit events are moved here when the hot
    /// window grows beyond `VOUCH_AUDIT_TRAIL_ARCHIVE_TRIGGER_ENTRIES`.
    ArchivedVouchAuditTrail(Address, Address, Address, u32),
    /// (borrower, voucher, token) → u32 number of archive batches created so
    /// far for this relationship's audit trail. The index needed to
    /// enumerate `ArchivedVouchAuditTrail` batches in order (0..count).
    VouchAuditTrailArchiveCount(Address, Address, Address),
    // ── Vouch splitting (Issue #1167) ────────────────────────────────────────
    /// borrower → Vec<VouchSplitRecord> genealogy of every split performed
    /// against a vouch for this borrower (parent voucher → child voucher).
    VouchSplitHistory(Address),
    // ── Vouch rotation incentives (Issue #1165) ──────────────────────────────
    /// voucher → u64 ledger timestamp of the voucher's most recent rotation.
    LastRotationTimestamp(Address),
    /// voucher → u32 total number of rotations performed by this voucher.
    RotationCount(Address),
    /// voucher → u32 basis-point yield bonus earned from quarterly rotation.
    RotationBonusBps(Address),
    // ── Vouch portfolio risk (Issue #1164) ───────────────────────────────────
    /// voucher → Vec<PortfolioSnapshot> historical evolution of the voucher's
    /// portfolio, appended each time the portfolio risk report is read.
    VoucherPortfolioHistory(Address),
    // ── Refinance rate shopping (Issue #1166) ────────────────────────────────
    /// Global aggregate statistics for `refinance_loan` usage.
    RefinanceStats,
    
    // ── Issue #967: Arbitrage Prevention ──────────────────────────────────
    /// (token_a, token_b) → ExchangeRate
    ExchangeRate(Address, Address),
    /// (token_a, token_b) → RateHistory
    RateHistory(Address, Address),
    /// (token_a, token_b) → PendingRateUpdate (two-step rate change, Issue #1431)
    PendingRateUpdate(Address, Address),
    /// Global u64: max age of `RateHistory` before min/max decay (Issue #1433)
    RateHistoryWindowSecs,
    
    // ── Issue #970: Cross-Chain Governance ────────────────────────────────
    /// proposal_id → CrossChainProposal
    CrossChainProposal(u64),
    /// (proposal_id, voter) → CrossChainVote
    CrossChainVote(u64, Address),
    /// (origin_chain, nonce) → true once a vote attestation with that nonce has been consumed.
    VoteAttestationNonceUsed(u32, u64),
    /// (chain_id, nonce) → true once a `submit_cross_chain_vote` call with that
    /// (chain, nonce) pair has been processed. Prevents the same voter's weight
    /// from a given origin chain being resubmitted and double-counted.
    CrossChainVoteNonceUsed(u32, u64),

    // ── Issue #974: Cross-Chain Auction ───────────────────────────────────
    /// auction_id → CrossChainAuction
    CrossChainAuction(u64),
    /// (auction_id, bidder) → Bid
    AuctionBid(u64, Address),
    /// auction_id → AuctionSettlement
    AuctionSettlement(u64),
    
    // ── Issue #978: Liquidity Farming ─────────────────────────────────────
    /// pool_id → LiquidityFarmPool
    FarmPool(u64),
    /// (pool_id, lp_provider) → FarmingPosition
    FarmingPosition(u64, Address),
    // ── Liquidity Mining Campaigns (Issue #1257) ─────────────────────────────
    /// campaign_id → MiningCampaign
    MiningCampaign(u64),
    /// Monotonically increasing campaign ID counter
    MiningCampaignCounter,
    /// (campaign_id, participant) → i128 reward claimed so far
    MiningClaimed(u64, Address),
    /// (campaign_id, participant) → i128 total participation (stake-seconds accumulated)
    MiningParticipation(u64, Address),
    // ── Issue #1074-1077: Multi-Token and Bridge Support ────────────────────
    /// token_addr → u32 liquidity tier (0-3) for dynamic yield bonuses
    TokenLiquidityTier(Address),
    /// token_addr → TokenBridgeMetadata for bridged tokens
    BridgedTokenMetadata(Address),
    /// source_token → i128 balance of bridged tokens held by the contract
    BridgedTokenBalance(Address),
    /// token_addr → u32 price in basis points relative to native token
    BridgeTokenPrice(Address),
    /// Reentrancy guard for token transfer operations
    ReentrancyGuard,
    // ── Issue #1070: Circuit Breaker for Rapid Default Cascade ─────────────────
    /// Timestamp (u64) when the circuit breaker was last triggered (activated).
    /// Used to enforce cooldown between successive circuit-breaker activations.
    CircuitBreakerLastTriggered,
    /// Default rate threshold (u32) in basis points at which the circuit breaker activates.
    /// Stored separately to allow runtime updates via governance.
    DefaultRateThreshold,
    // ── Issue #1071: Insurance Fund Mechanism ──────────────────────────────────
    /// Balance of the protocol's dedicated insurance fund (i128 stroops).
    /// Pre-funded by admin or protocol fees; drawn down to cover slash shortfalls.
    InsuranceFund,
    /// Timestamp (u64) of the most recent insurance fund contribution.
    InsuranceFundLastContribution,

    // ── Issue #1172: Guarantor system ───────────────────────────────────────
    /// loan_id → GuarantorRecord
    GuarantorRecord(u64),
    /// (guarantor, loan_id) → GuarantorObligation
    GuarantorObligation(Address, u64),
    /// guarantor → GuarantorStats
    GuarantorStats(Address),

    // ── Issue #1238: Staking Pool ────────────────────────────────────────────
    /// pool_id → StakingPool
    StakingPool(u64),
    /// u64: monotonically increasing staking pool ID counter
    StakingPoolCounter,
    /// (pool_id, staker) → StakerPosition
    StakingPoolStake(u64, Address),

    // ── Vouch syndication ─────────────────────────────────────────────────────
    /// pool_id → SyndicatePool
    SyndicatePool(u64),
    /// (pool_id, member) → SyndicateMember
    SyndicateMember(u64, Address),
    /// pool_id → SyndicatePerformance
    SyndicatePerformance(u64),
    /// pool_id → u64: monotonically increasing proposal ID counter for that pool
    SyndicateProposalCounter(u64),
    /// (pool_id, proposal_id) → SyndicateProposal
    SyndicateProposal(u64, u64),
    /// (pool_id, proposal_id, voter) → bool: has this member voted
    SyndicateProposalVote(u64, u64, Address),

    // ── Issue #1183: Flash loans ─────────────────────────────────────────────
    /// Aggregate flash loan statistics (volume, fees, count)
    FlashLoanStats,
    /// contract → PerContractCap: per-contract flash-loan borrowing cap state
    FlashLoanPerContractCap(Address),
    /// Recent flash loan activity records (bounded ring buffer)
    FlashLoanHistory,
    /// contract → u64: ledger timestamp of that contract's last flash loan
    FlashLoanLastTimestamp(Address),
    /// contract → bool: whether this callback contract is allowed to receive flash loans
    AllowedFlashLoanCallbacks(Address),

    // ── Cross-chain / multi-token bridge ─────────────────────────────────────
    /// token → i128: bridged balance for that token
    BridgedTokenBalance(Address),
    /// token → u32: bridge conversion price in basis points
    BridgeTokenPrice(Address),
    /// token → TokenBridgeMetadata
    TokenBridgeMetadata(Address),
    /// Reentrancy guard lock (u32: 0 = unlocked, 1 = locked)
    ReentrancyGuard,
    /// loan_id → TokenSwapConfig
    LoanTokenSwapConfig(u64),
    /// Address of the configured DEX contract used for token swaps
    DexContractAddress,
    /// token → u32: liquidity tier for that token
    TokenLiquidityTier(Address),
    /// Vec<i128>: yield bonus (bps) per liquidity tier
    LiquidityTierYieldBonuses,

    // ── Weighted vouch reputation ─────────────────────────────────────────────
    /// vouch_id → weight record
    VouchReputationWeight(u64),
    /// (borrower, token) → WeightedVouchDistribution
    WeightedVouchDistribution(Address, Address),

    // ── Issue #1169: Milestone-based vouch release ───────────────────────────
    /// (loan_id, voucher, milestone_index) → bool: has this release been paid
    VouchMilestoneRelease(u64, Address, u32),
    /// (loan_id, milestone_index) → bool: has this milestone been achieved
    MilestoneAchieved(u64, u32),

    // ── Recurring payments ────────────────────────────────────────────────────
    /// borrower → RecurringPaymentConfig
    RecurringPayment(Address),

    // ── Issue #1247: Referral rewards ─────────────────────────────────────────
    /// referrer → i128: total referral rewards earned
    ReferralRewardsEarned(Address),
    /// referrer → BytesN<32>: referral code hash (lookup by owner)
    ReferralCode(Address),
    /// code hash → Address: referrer address (reverse lookup)
    ReferralCodeOwner(BytesN<32>),
    /// referrer → u32: number of successful referrals
    ReferralCount(Address),

    // ── Reputation badges (NFT-style achievements) ───────────────────────────
    /// (owner, badge_type) → Badge
    ReputationBadge(Address, BadgeType),
    /// badge_type → BadgeStats
    BadgeStats(BadgeType),
    /// address → u32: reputation score
    ReputationScore(Address),
    /// address → u32: number of vouches this address has backed
    VoucherBackedCount(Address),

    // ── Prediction markets ────────────────────────────────────────────────────
    /// u64: monotonically increasing prediction market ID counter
    PredictionMarketCounter,
    /// market_id → PredictionMarket
    PredictionMarket(u64),
    /// (market_id, participant) → MarketPosition
    MarketPosition(u64, Address),
    /// participant → PredictionAccuracy
    PredictionAccuracy(Address),

    // ── Community treasury / DAO ─────────────────────────────────────────────
    /// i128: current treasury balance
    TreasuryBalance,
    /// u64: monotonically increasing treasury proposal ID counter
    TreasuryProposalCounter,
    /// proposal_id → TreasuryProposal
    TreasuryProposal(u64),
    /// (proposal_id, voter) → bool: has this address voted
    TreasuryVote(u64, Address),
    /// month_id → TreasuryReport
    TreasuryReport(u64),

    // ── Governance token / DAO proposals ─────────────────────────────────────
    /// Aggregate governance participation metrics
    GovParticipationMetrics,
    /// holder → i128: governance token balance
    GovTokenBalance(Address),
    /// u64: monotonically increasing DAO proposal ID counter
    DaoProposalCounter,
    /// delegator → GovDelegation
    GovDelegation(Address),
    /// proposal_id → DaoProposal
    DaoProposal(u64),

    // ── Interest rate options ─────────────────────────────────────────────────
    /// u32: implied volatility in basis points per day
    ImpliedVolatility,
    /// u64: monotonically increasing option ID counter
    OptionCounter,
    /// option_id → InterestRateOption
    InterestRateOption(u64),
    /// option_type → OptionOpenInterest
    OptionOpenInterest(OptionType),

    // ── Dynamic interest rate ─────────────────────────────────────────────────
    /// Utilization-rate model configuration
    UtilizationRateConfig,
    /// Latest computed utilization-rate snapshot
    UtilizationRateSnapshot,

    // ── Loyalty program ───────────────────────────────────────────────────────
    /// user → LoyaltyRecord
    LoyaltyRecord(Address),

    // ── Protocol-wide aggregate counters ─────────────────────────────────────
    /// u32: total number of currently active loans
    TotalActiveLoans,
    /// i128: total value locked across the protocol
    TotalValueLocked,
    /// Vec<Address>: registry of all addresses that have ever vouched
    VoucherRegistry,

    // ── Issue #1080: Request idempotency ─────────────────────────────────────
    /// idempotency_key → IdempotencyRecord
    IdempotencyKey(String),
    /// (user, role) → rate limit tracking state
    RateLimitByRole(Address, UserRole),

    // ── Issue #1361: Cross-Chain Relay Pipeline ──────────────────────────────
    /// source_chain → Ed25519 public key trusted to sign relay messages
    RelayPublicKey(u32),
    /// (source_chain, nonce) → bool: has this nonce been consumed
    RelayNonceUsed(u32, u64),
    /// (dest_chain, seq) → RelayEvent: outbound event stored for retrieval
    OutboundRelayEvent(u32, u64),
    /// dest_chain → u64: latest outbound sequence number for that chain
    OutboundRelaySeq(u32),
    /// dest_chain → u64: last acknowledged outbound sequence (for delivery tracking)
    LastAcknowledgedRelaySeq(u32),
    /// (source_chain, seq) → bool: has this inbound event been processed
    RelayEventProcessed(u32, u64),

    // ── Issue #10: Refinance chain limits ────────────────────────────────────
    /// borrower → u32: how many refinances have been chained off the original loan
    RefinanceChainCount(Address),
    /// borrower → u64: timestamp of the most recent refinance
    LastRefinancedAt(Address),
    // Issue #111: Per-subject webhook subscription limit override
    WebhookLimit,
    // Issue #112: Off-chain sub-system health sentinels
    PubSubHealthy,              // bool: true when PubSub relay last checked in successfully
    RevocationStoreHealthy,     // bool: true when RevocationStore proxy last checked in
    WebhookRegistryHealthy,     // bool: true when WebhookRegistry proxy last checked in

    // ── Issue #1607-1611: Attestor Analytics and Quorum Slice Monitoring ───────
    /// slice_id → Vec<Address> members of this quorum slice
    AttestorSliceMembers(u64),
    /// (attestor_a, attestor_b) → ConcordanceRecord for collusion detection
    AttestorConcordance(Address, Address),
    /// (attestor_a, attestor_b, timestamp) → (concordance_score, ledger) collusion alert
    CollusionAlert(Address, Address, u64),
    /// Last ledger at which collusion checks were performed
    LastCollusionCheckLedger,
    /// attestor → AttestorAvailability tracking record
    AttestorAvailability(Address),
    /// (attestor, timestamp) → availability_percentage low-availability alert
    AvailabilityAlert(Address, u64),
    /// slice_id → Vec<SlicePerformanceRecord> historical performance data
    SlicePerformanceHistory(u64),
    /// attestor → AttestorLocation geographic metadata
    AttestorLocation(Address),

    // ── Issue #1738: SBT Reputation System ───────────────────────────────────
    /// holder → SbtReputationRecord
    SbtReputation(Address),

    // ── Issue #1739: SBT Group Ownership ──────────────────────────────────────
    /// u64: monotonically increasing group SBT ID counter
    SbtGroupCounter,
    /// group_id → GroupSbt
    SbtGroup(u64),
    /// (group_id, proposal_id) → GroupMembershipProposal
    SbtGroupProposal(u64, u64),

    // ── Issue #1740: SBT Staking and Rewards ─────────────────────────────────
    /// (sbt_id, holder) → SbtStakeRecord
    SbtStake(u64, Address),

    // ── Issue #1741: SBT Lineage Tracking ─────────────────────────────────────
    /// sbt_id → SbtLineageNode
    SbtLineage(u64),

    // ── Issue #1733: SBT Attribute Evolution ─────────────────────────────────
    /// (sbt_id, attribute_key) → SbtAttributeRecord
    SbtAttribute(u64, soroban_sdk::Bytes),

    // ── Issue #1734: SBT Privacy Modes ───────────────────────────────────────
    /// sbt_id → SbtPrivacyRecord
    SbtPrivacy(u64),

    // ── Issue #1735: SBT Insurance Coverage ──────────────────────────────────
    /// sbt_id → SbtInsuranceRecord
    SbtInsurance(u64),

    // ── Issue #1736: SBT Marketplace Integration ──────────────────────────────
    /// sbt_id → SbtMarketplaceListing
    SbtMarketplaceListing(u64),
    /// Global marketplace index: Vec<u64>
    SbtMarketplaceIndex,
    /// Global marketplace statistics: SbtMarketplaceStats
    SbtMarketplaceStats,
}

/// Issue #867: Shared collateral pool backed by multiple vouchers.
#[contracttype]
#[derive(Clone)]
pub struct CollateralPool {
    pub pool_id: u64,
    pub members: Vec<Address>,
    /// Stake per member (parallel to `members`), in stroops.
    pub stakes: Vec<i128>,
    /// Origin chain per member (parallel to `members`). `0` is the native chain.
    pub chain_ids: Vec<u32>,
    pub token: Address,
    pub borrower: Option<Address>,
    pub active: bool,
    pub created_at: u64,
}

// ── Liquidity Mining (Issue #1257) ────────────────────────────────────────────

/// Campaign type governing how rewards are distributed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MiningCampaignType {
    /// Rewards proportional to each participant's stake contribution.
    ProportionalStake,
    /// Flat reward per unique participating voucher (equal-split).
    FlatPerVoucher,
    /// Rewards proportional to voucher reputation score.
    ReputationWeighted,
}

/// Lifecycle state of a liquidity mining campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MiningCampaignStatus {
    /// Accepting participants; rewards not yet claimable.
    Active,
    /// Campaign ended; rewards are claimable.
    Ended,
    /// Campaign was cancelled before ending; no rewards disbursed.
    Cancelled,
}

/// Issue #1257: A liquidity mining campaign record stored on-chain.
///
/// Campaigns bootstrap liquidity by distributing rewards from `incentive_pool`
/// to vouchers who participate during `[start_timestamp, end_timestamp)`.
/// The reward each participant earns depends on the `campaign_type`.
#[contracttype]
#[derive(Clone)]
pub struct MiningCampaign {
    /// Unique campaign identifier (1-indexed monotonic counter).
    pub campaign_id: u64,
    /// Creator/sponsor of the campaign (must be an admin).
    pub creator: Address,
    /// Token used for both participation tracking and reward payout.
    pub token: Address,
    /// Total reward tokens deposited into the campaign pool, in stroops.
    pub incentive_pool: i128,
    /// Reward tokens already distributed so far, in stroops.
    pub distributed: i128,
    /// Campaign start ledger timestamp (inclusive).
    pub start_timestamp: u64,
    /// Campaign end ledger timestamp (exclusive).
    pub end_timestamp: u64,
    /// Distribution algorithm.
    pub campaign_type: MiningCampaignType,
    /// Lifecycle status.
    pub status: MiningCampaignStatus,
    /// Total accumulated participation weight (stake-seconds or voucher count).
    pub total_participation: i128,
    /// Number of unique participants who have recorded participation.
    pub participant_count: u64,
}

// ── Issue #1238: Staking Pool with Yield Farming ──────────────────────────────

/// Lifecycle state of a staking pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StakingPoolStatus {
    /// Pool is open and accepting stakes.
    Active,
    /// Pool is processing withdrawals; no new stakes accepted.
    Draining,
    /// Pool is closed; all stakes have been returned.
    Closed,
}

/// Issue #1238: A yield-bearing staking pool that lets capital holders earn
/// returns sourced from the protocol's lending operations.
///
/// Stakers deposit tokens and receive a proportional share of yield accrued
/// from the lending yield reserve over time.  Withdrawals are queued to
/// prevent bank-run dynamics; the queue is drained when yield is collected.
#[contracttype]
#[derive(Clone)]
pub struct StakingPool {
    /// Unique pool identifier (1-indexed monotonic counter).
    pub pool_id: u64,
    /// Token staked in this pool (must be the protocol token or an allowed token).
    pub token: Address,
    /// Total tokens currently deposited by all stakers, in stroops.
    pub total_staked: i128,
    /// Accumulated yield per stroop (scaled by 1e12 for precision).
    /// Updated each time yield is distributed from the lending reserve.
    pub yield_per_token_scaled: i128,
    /// Annual Percentage Yield in basis points, computed lazily on each distribution.
    pub current_apy_bps: u32,
    /// Total yield distributed to stakers since pool creation, in stroops.
    pub total_yield_distributed: i128,
    /// Timestamp of the last yield distribution event.
    pub last_yield_timestamp: u64,
    /// Pool lifecycle status.
    pub status: StakingPoolStatus,
    /// Timestamp when the pool was created.
    pub created_at: u64,
}

/// Issue #1238: Per-staker position in a staking pool.
/// Stored under `DataKey::StakingPoolStake(pool_id, staker)`.
#[contracttype]
#[derive(Clone)]
pub struct StakerPosition {
    /// Staker address.
    pub staker: Address,
    /// Amount currently staked, in stroops.
    pub amount: i128,
    /// Snapshot of `yield_per_token_scaled` at time of last claim/stake.
    /// Used to compute pending rewards: (current - snapshot) * amount / 1e12.
    pub yield_snapshot_scaled: i128,
    /// Accumulated rewards not yet withdrawn, in stroops.
    pub pending_rewards: i128,
    /// Timestamp of the staker's last action (stake/unstake/claim).
    pub last_action_timestamp: u64,
    /// Whether there is a pending unstake in the withdrawal queue.
    pub pending_unstake: bool,
    /// Amount queued for unstaking (0 when `pending_unstake` is false).
    pub queued_unstake_amount: i128,
}

// ── Issue #1247: Referral Rewards Program ─────────────────────────────────────

/// Issue #1247: Referral leaderboard entry for a single referrer.
#[contracttype]
#[derive(Clone)]
pub struct ReferralStats {
    /// The referrer's address.
    pub referrer: Address,
    /// Number of referred borrowers who have completed at least one loan.
    pub conversion_count: u64,
    /// Total referral rewards earned (in stroops).
    pub total_rewards_earned: i128,
    /// Timestamp of the most recent referral conversion.
    pub last_conversion_at: u64,
}

// ── Governance ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct SlashVoteRecord {
    /// Total stake (in stroops) that has voted to approve this slash.
    /// 1 XLM = 10,000,000 stroops.
    pub approve_stake: i128,
    /// Total stake (in stroops) that has voted to reject this slash.
    /// 1 XLM = 10,000,000 stroops.
    pub reject_stake: i128,
    /// Addresses that have already cast a vote on this proposal.
    pub voters: Vec<Address>,
    /// `true` once the slash has been auto-executed after quorum was reached.
    pub executed: bool,
}

/// Slash escrow record holding slashed funds in 7-day escrow pending appeal.
#[contracttype]
#[derive(Clone)]
pub struct SlashEscrow {
    pub borrower: Address,
    pub loan_id: u64,
    /// Slashed amount held in escrow (50% of total stake).
    pub escrow_amount: i128,
    /// Timestamp when escrow period expires (created_at + 7 days).
    pub release_timestamp: u64,
    /// Status: Pending, Approved, or Rejected.
    pub status: AppealStatus,
}

/// Status of a slash appeal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppealStatus {
    /// Appeal in progress.
    Pending,
    /// Appeal approved; slash is overturned and funds returned to vouchers.
    Approved,
    /// Appeal rejected; funds are burned after escrow period.
    Rejected,
}

/// Record of a slash appeal voted on by vouchers (Issue #841: escrow-based).
#[contracttype]
#[derive(Clone)]
pub struct SlashEscrowAppealRecord {
    pub borrower: Address,
    pub loan_id: u64,
    /// Total stake that voted to approve the appeal (overturn slash).
    pub approve_stake: i128,
    /// Total stake that voted to reject the appeal (keep slash).
    pub reject_stake: i128,
    /// Addresses that have already voted on this appeal.
    pub voters: Vec<Address>,
    /// Timestamp when appeal was created.
    pub appeal_timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct DisputeRecord {
    pub borrower: Address,
    pub voucher: Address,
    pub evidence_hash: soroban_sdk::BytesN<32>,
    pub disputed_at: u64,
    pub resolved: Option<bool>,
}

/// Governance proposal to change the protocol slash threshold (`Config.slash_bps`).
#[contracttype]
#[derive(Clone)]
pub struct SlashThresholdProposal {
    pub id: u64,
    pub proposer: Address,
    pub proposed_threshold: i128,
    pub proposed_at: u64,
    pub approve_votes: u32,
    pub reject_votes: u32,
    pub voters: Vec<Address>,
    pub finalized: bool,
}

/// Config field targeted by an admin config-update proposal.
#[contracttype]
#[derive(Clone)]
pub enum ConfigUpdateKey {
    AdminThreshold,
}

/// Multi-sig admin proposal to update a config field.
#[contracttype]
#[derive(Clone)]
pub struct ConfigUpdateProposal {
    pub id: u64,
    pub proposer: Address,
    pub key: ConfigUpdateKey,
    pub new_value: u32,
    pub approvals: Vec<Address>,
    pub executed: bool,
}

// ── Admin Governance Queue with Multi-Signature Confirmation ─────────────────────

/// Issue #893: Admin operation types for multi-tier approval thresholds.
/// Different operations can require different numbers of admin approvals based on criticality.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminOperationType {
    /// Low-risk operations (e.g., setting parameters like min_stake)
    Standard,
    /// Medium-risk operations (e.g., adding/removing tokens, admin changes)
    HighRisk,
    /// Critical operations (e.g., contract upgrade, pause, emergency actions)
    Critical,
}

/// Issue #893: Multi-tier admin approval thresholds for different operation types.
/// Allows different admin operations to require different numbers of approvals.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiTierAdminThresholds {
    /// Approvals required for standard operations (default: same as admin_threshold)
    pub standard_threshold: u32,
    /// Approvals required for high-risk operations (default: 2x standard)
    pub high_risk_threshold: u32,
    /// Approvals required for critical operations (default: all admins)
    pub critical_threshold: u32,
}

impl MultiTierAdminThresholds {
    /// Create default thresholds based on total admin count.
    /// Standard = 1, HighRisk = (total/2)+1, Critical = total
    pub fn default_for_admin_count(admin_count: u32) -> Self {
        let high_risk = if admin_count > 1 { (admin_count / 2) + 1 } else { 1 };
        let critical = admin_count;
        MultiTierAdminThresholds {
            standard_threshold: 1,
            high_risk_threshold: high_risk,
            critical_threshold: critical,
        }
    }

    /// Get the threshold for a specific operation type
    pub fn get_threshold(&self, operation_type: AdminOperationType) -> u32 {
        match operation_type {
            AdminOperationType::Standard => self.standard_threshold,
            AdminOperationType::HighRisk => self.high_risk_threshold,
            AdminOperationType::Critical => self.critical_threshold,
        }
    }
}

/// Types of governance actions that can be proposed in the admin governance queue.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GovernanceAction {
    /// Pause the contract
    Pause,
    /// Unpause the contract
    Unpause,
    /// Upgrade the contract to a new WASM hash
    Upgrade(BytesN<32>),
    /// Set protocol fee in basis points
    SetProtocolFee(u32),
    /// Set fee treasury address
    SetFeeTreasury(Address),
    /// Add an allowed token
    AddAllowedToken(Address),
    /// Remove an allowed token
    RemoveAllowedToken(Address),
    /// Set minimum stake amount
    SetMinStake(i128),
    /// Set maximum loan amount
    SetMaxLoanAmount(i128),
    /// Set minimum vouchers required
    SetMinVouchers(u32),
    /// Set maximum vouchers per borrower
    SetMaxVouchersPerBorrower(u32),
    /// Set max loan to stake ratio
    SetMaxLoanToStakeRatio(u32),
    /// Set grace period
    SetGracePeriod(u64),
    /// Set yield basis points
    SetYieldBps(i128),
    /// Set slash basis points
    SetSlashBps(i128),
    /// Set admin threshold
    SetAdminThreshold(u32),
    /// Add an admin
    AddAdmin(Address),
    /// Remove an admin
    RemoveAdmin(Address),
    /// Rotate an admin
    RotateAdmin(Address, Address),
    /// Set reputation NFT contract
    SetReputationNft(Address),
    /// Set whitelist enabled
    SetWhitelistEnabled(bool),
    /// Blacklist a borrower
    BlacklistBorrower(Address),
    /// Set prepayment penalty basis points
    SetPrepaymentPenaltyBps(u32),
    /// Set dynamic slash threshold enabled
    SetDynamicSlashThreshold(bool),
    /// Set loan size slash enabled
    SetLoanSizeSlashEnabled(bool),
    /// Set loan size slash max basis points
    SetLoanSizeSlashMaxBps(i128),
    /// Set successor admin
    SetSuccessorAdmin(Option<Address>),
    /// Set confirmation required
    SetConfirmationRequired(bool),
    /// Set admin compensation basis points
    SetAdminCompensationBps(u32),
    /// Set removal vote threshold
    SetRemovalVoteThreshold(u32),
    /// Set rate limit config
    SetRateLimitConfig(RateLimitConfig),
}

/// Status of a governance proposal in the queue.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GovernanceProposalStatus {
    /// Proposal is pending approval
    Pending,
    /// Proposal has been approved and can be executed
    Approved,
    /// Proposal has been executed
    Executed,
    /// Proposal has been cancelled
    Cancelled,
    /// Proposal has expired
    Expired,
}

/// A governance proposal in the admin governance queue with multi-signature confirmation.
#[contracttype]
#[derive(Clone)]
pub struct GovernanceProposal {
    /// Unique proposal ID
    pub id: u64,
    /// The governance action to be executed
    pub action: GovernanceAction,
    /// Address that proposed the action
    pub proposer: Address,
    /// Addresses that have approved this proposal
    pub approvals: Vec<Address>,
    /// Addresses that have rejected this proposal
    pub rejections: Vec<Address>,
    /// Current status of the proposal
    pub status: GovernanceProposalStatus,
    /// Ledger timestamp when the proposal was created
    pub created_at: u64,
    /// Ledger timestamp when the proposal can be executed (timelock)
    pub executable_at: u64,
    /// Ledger timestamp when the proposal expires (if not executed)
    pub expires_at: u64,
    /// Optional description or justification for the proposal
    pub description: soroban_sdk::String,
    /// Ledger timestamp when the proposal was executed (if applicable)
    pub executed_at: Option<u64>,
}

/// Governance queue configuration parameters.
#[contracttype]
#[derive(Clone)]
pub struct GovernanceQueueConfig {
    /// Minimum delay before a proposal can be executed (in seconds)
    pub timelock_delay: u64,
    /// Time window after executable_at during which a proposal can be executed (in seconds)
    pub execution_window: u64,
    /// Whether proposals require multi-sig approval (true) or can be executed by proposer (false)
    pub require_multisig: bool,
}

/// Default timelock delay for governance proposals (24 hours).
pub const DEFAULT_GOVERNANCE_TIMELOCK_DELAY: u64 = 24 * 60 * 60;

/// Default execution window for governance proposals (7 days).
pub const DEFAULT_GOVERNANCE_EXECUTION_WINDOW: u64 = 7 * 24 * 60 * 60;

// ── On-Chain Credit Score with Tiered Rewards ─────────────────────────────────────

/// Credit score tier levels.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreditTier {
    /// Tier 1: Poor (0-349)
    Poor,
    /// Tier 2: Fair (350-549)
    Fair,
    /// Tier 3: Good (550-699)
    Good,
    /// Tier 4: Very Good (700-849)
    VeryGood,
    /// Tier 5: Excellent (850-1000)
    Excellent,
}

/// Comprehensive credit score record for a borrower.
#[contracttype]
#[derive(Clone)]
pub struct CreditScore {
    /// Overall credit score (0-1000)
    pub score: u32,
    /// Current credit tier
    pub tier: CreditTier,
    /// Ledger timestamp when the score was last updated
    pub last_updated: u64,
    /// Ledger timestamp when the score was last decayed (Issue #1072)
    pub last_decay_timestamp: u64,
    /// Total number of loans taken
    pub total_loans: u32,
    /// Number of successfully repaid loans
    pub successful_repayments: u32,
    /// Number of defaults
    pub defaults: u32,
    /// Total amount borrowed (in stroops)
    pub total_borrowed: i128,
    /// Total amount repaid (in stroops)
    pub total_repaid: i128,
    /// Account age in seconds
    pub account_age: u64,
    /// Number of times as a voucher
    pub voucher_count: u32,
    /// Average repayment time (in seconds before deadline, negative if late)
    pub avg_repayment_time: i64,
}

// ── zk-SNARK Confidentiality Types ─────────────────────────────────────────────

/// A zk-SNARK proof for confidential operations
#[contracttype]
#[derive(Clone)]
pub struct ZkProof {
    /// Proof points (compressed representation)
    pub proof_bytes: soroban_sdk::Bytes,
    /// Public inputs for the proof
    pub public_inputs: soroban_sdk::Vec<soroban_sdk::BytesN<32>>,
    /// Proof type identifier
    pub proof_type: u32,
}

/// A commitment to a confidential value using hash-based commitment
#[contracttype]
#[derive(Clone)]
pub struct ConfidentialCommitment {
    /// The commitment value (hash of the confidential amount and a prover-side blinding factor).
    /// The blinding factor never enters on-chain storage.
    pub commitment: soroban_sdk::BytesN<32>,
}

/// Public parameters for the zk-SNARK system
#[contracttype]
#[derive(Clone)]
pub struct ZkPublicParams {
    /// Verifying key hash (for on-chain verification)
    pub vk_hash: soroban_sdk::BytesN<32>,
    /// Circuit identifier
    pub circuit_id: u32,
}

/// Audit record for a zk-SNARK proof
#[contracttype]
#[derive(Clone)]
pub struct ZkProofRecord {
    /// Unique proof ID
    pub proof_id: u64,
    /// The proof that was verified
    pub proof: ZkProof,
    /// Operation type (vouch, loan_request, repayment)
    pub operation_type: u32,
    /// Address that submitted the proof
    pub submitter: Address,
    /// Whether verification succeeded
    pub verified: bool,
    /// Ledger timestamp when proof was submitted
    pub submitted_at: u64,
}

/// Proof types for different operations
pub const PROOF_TYPE_VOUCH: u32 = 1;
pub const PROOF_TYPE_LOAN_REQUEST: u32 = 2;
pub const PROOF_TYPE_REPAYMENT: u32 = 3;

/// Error for invalid proof type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZkError {
    InvalidProofType,
    ProofVerificationFailed,
    InvalidCommitment,
}

/// Reputation NFT badge record for borrowers reaching Excellent tier.
#[contracttype]
#[derive(Clone)]
pub struct ReputationNFTRecord {
    /// Address of the borrower who minted the badge
    pub borrower: Address,
    /// Ledger timestamp when the NFT badge was minted
    pub minted_at: u64,
}

/// Credit score calculation factors.
#[contracttype]
#[derive(Clone)]
pub struct CreditFactors {
    /// Weight for repayment history (0-10000 basis points)
    pub repayment_history_weight: u32,
    /// Weight for loan count (0-10000 basis points)
    pub loan_count_weight: u32,
    /// Weight for account age (0-10000 basis points)
    pub account_age_weight: u32,
    /// Weight for vouching activity (0-10000 basis points)
    pub vouching_weight: u32,
    /// Weight for repayment timeliness (0-10000 basis points)
    pub timeliness_weight: u32,
}

/// Tiered reward benefits for each credit tier.
#[contracttype]
#[derive(Clone)]
pub struct TierRewards {
    /// Yield basis points bonus (added to base yield)
    pub yield_bonus_bps: i32,
    /// Maximum loan amount multiplier (e.g., 150 = 1.5x)
    pub max_loan_multiplier: u32,
    /// Minimum stake reduction in basis points (e.g., 1000 = 10% reduction)
    pub min_stake_reduction_bps: u32,
    /// Loan duration extension in seconds (e.g., 7 days = 604800)
    pub duration_extension: u64,
    /// Fee discount in basis points (e.g., 500 = 5% discount)
    pub fee_discount_bps: u32,
}

/// Credit score configuration parameters.
#[contracttype]
#[derive(Clone)]
pub struct CreditScoreConfig {
    /// Whether credit scoring is enabled
    pub enabled: bool,
    /// Credit score calculation factors
    pub factors: CreditFactors,
    /// Rewards for each tier
    pub poor_rewards: TierRewards,
    pub fair_rewards: TierRewards,
    pub good_rewards: TierRewards,
    pub very_good_rewards: TierRewards,
    pub excellent_rewards: TierRewards,
}

/// Default credit score factors.
pub const DEFAULT_CREDIT_FACTORS: CreditFactors = CreditFactors {
    repayment_history_weight: 4000,  // 40%
    loan_count_weight: 1500,         // 15%
    account_age_weight: 1000,         // 10%
    vouching_weight: 1500,            // 15%
    timeliness_weight: 2000,          // 20%
};

/// Default tier rewards configuration.
pub const DEFAULT_POOR_REWARDS: TierRewards = TierRewards {
    yield_bonus_bps: 0,
    max_loan_multiplier: 100,
    min_stake_reduction_bps: 0,
    duration_extension: 0,
    fee_discount_bps: 0,
};

pub const DEFAULT_FAIR_REWARDS: TierRewards = TierRewards {
    yield_bonus_bps: 50,
    max_loan_multiplier: 110,
    min_stake_reduction_bps: 500,
    duration_extension: 86400,      // 1 day
    fee_discount_bps: 100,
};

pub const DEFAULT_GOOD_REWARDS: TierRewards = TierRewards {
    yield_bonus_bps: 100,
    max_loan_multiplier: 125,
    min_stake_reduction_bps: 1000,
    duration_extension: 172800,     // 2 days
    fee_discount_bps: 250,
};

pub const DEFAULT_VERY_GOOD_REWARDS: TierRewards = TierRewards {
    yield_bonus_bps: 150,
    max_loan_multiplier: 150,
    min_stake_reduction_bps: 1500,
    duration_extension: 345600,     // 4 days
    fee_discount_bps: 500,
};

pub const DEFAULT_EXCELLENT_REWARDS: TierRewards = TierRewards {
    yield_bonus_bps: 200,
    max_loan_multiplier: 200,
    min_stake_reduction_bps: 2000,
    duration_extension: 604800,     // 7 days
    fee_discount_bps: 1000,
};

/// Default credit score configuration.
pub const DEFAULT_CREDIT_SCORE_CONFIG: CreditScoreConfig = CreditScoreConfig {
    enabled: true,
    factors: DEFAULT_CREDIT_FACTORS,
    poor_rewards: DEFAULT_POOR_REWARDS,
    fair_rewards: DEFAULT_FAIR_REWARDS,
    good_rewards: DEFAULT_GOOD_REWARDS,
    very_good_rewards: DEFAULT_VERY_GOOD_REWARDS,
    excellent_rewards: DEFAULT_EXCELLENT_REWARDS,
};

// ── Loan Pool Syndication for Multi-Borrower Loans ───────────────────────────────

/// Syndication role for a member in a loan syndicate.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyndicationRole {
    /// Lead borrower - primary contact and decision maker
    LeadBorrower,
    /// Co-borrower - shares loan responsibility
    CoBorrower,
    /// Guarantor - provides additional collateral but not a borrower
    Guarantor,
}

/// Syndication member information.
#[contracttype]
#[derive(Clone)]
pub struct SyndicationMember {
    /// Member address
    pub address: Address,
    /// Role in the syndication
    pub role: SyndicationRole,
    /// Share of the loan (in basis points, e.g., 5000 = 50%)
    pub share_bps: u32,
    /// Collateral contributed (in stroops)
    pub collateral: i128,
    /// Vouches contributed (stake amount in stroops)
    pub vouch_stake: i128,
    /// Whether the member has approved the syndication
    pub approved: bool,
    /// Ledger timestamp when the member joined
    pub joined_at: u64,
}

/// Loan syndication record for multi-borrower loans.
#[contracttype]
#[derive(Clone)]
pub struct LoanSyndication {
    /// Unique syndication ID
    pub syndication_id: u64,
    /// Associated loan ID (if loan has been disbursed)
    pub loan_id: Option<u64>,
    /// Syndication members
    pub members: Vec<SyndicationMember>,
    /// Total loan amount requested (in stroops)
    pub total_amount: i128,
    /// Total collateral contributed (in stroops)
    pub total_collateral: i128,
    /// Total vouch stake (in stroops)
    pub total_vouch_stake: i128,
    /// Loan purpose description
    pub loan_purpose: soroban_sdk::String,
    /// Token address for the loan
    pub token_address: Address,
    /// Ledger timestamp when syndication was created
    pub created_at: u64,
    /// Ledger timestamp when syndication was disbursed (if applicable)
    pub disbursed_at: Option<u64>,
    /// Syndication status
    pub status: SyndicationStatus,
    /// Minimum number of approvals required
    pub min_approvals: u32,
    /// Current number of approvals
    pub approval_count: u32,
}

/// Syndication status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyndicationStatus {
    /// Syndication is being formed
    Forming,
    /// Syndication is ready for loan disbursement
    Ready,
    /// Loan has been disbursed
    Active,
    /// Loan has been fully repaid
    Repaid,
    /// Syndication has been cancelled
    Cancelled,
    /// Syndication has defaulted
    Defaulted,
}

/// Syndication repayment record.
#[contracttype]
#[derive(Clone)]
pub struct SyndicationRepayment {
    /// Syndication ID
    pub syndication_id: u64,
    /// Member who made the repayment
    pub repayer: Address,
    /// Amount repaid (in stroops)
    pub amount: i128,
    /// Ledger timestamp of repayment
    pub timestamp: u64,
}

/// Syndication configuration parameters.
#[contracttype]
#[derive(Clone)]
pub struct SyndicationConfig {
    /// Maximum number of members in a syndication
    pub max_members: u32,
    /// Minimum number of members required
    pub min_members: u32,
    /// Minimum approvals required (as percentage of members, e.g., 5000 = 50%)
    pub min_approval_percentage: u32,
    /// Maximum loan amount for syndication (in stroops)
    pub max_loan_amount: i128,
    /// Syndication fee in basis points (e.g., 100 = 1%)
    pub syndication_fee_bps: u32,
}

/// Default syndication configuration.
pub const DEFAULT_SYNDICATION_CONFIG: SyndicationConfig = SyndicationConfig {
    max_members: 10,
    min_members: 2,
    min_approval_percentage: 7500, // 75%
    max_loan_amount: 1_000_000_000_000, // 10 million XLM
    syndication_fee_bps: 100, // 1%
};

// ── Config ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admins: Vec<Address>,
    pub admin_threshold: u32,
    /// Admin addresses that are permitted to be configured as admins.
    /// If empty, any valid admin address may be used.
    pub admin_whitelist: Vec<Address>,
    /// Admin addresses that are explicitly forbidden from being configured as admins.
    pub admin_blacklist: Vec<Address>,
    /// Primary token contract address used for loans and vouches.
    pub token: Address,
    /// Additional token contract addresses accepted for loans/vouches.
    pub allowed_tokens: Vec<Address>,
    /// Yield rate in basis points (e.g. 200 = 2%). Applied to loan principal (in stroops)
    /// at repayment: `yield = principal_stroops * yield_bps / 10_000`.
    pub yield_bps: i128,
    /// Slash fraction in basis points (e.g. 5000 = 50%). Applied to voucher stake (in stroops)
    /// on borrower default: `slashed = stake_stroops * slash_bps / 10_000`.
    pub slash_bps: i128,
    pub max_vouchers: u32,
    /// Minimum loan amount, in stroops. 1 XLM = 10,000,000 stroops.
    pub min_loan_amount: i128,
    /// Maximum loan duration, in seconds.
    pub loan_duration: u64,
    /// Maximum ratio of loan amount to total staked collateral, expressed as a percentage
    /// (e.g. 150 means loan ≤ 1.5 × total stake in stroops).
    pub max_loan_to_stake_ratio: u32,
    pub max_loan_to_collateral_ratio: u32,
    /// Grace period after loan deadline before the loan can be slashed, in seconds.
    pub grace_period: u64,
    /// Minimum seconds between vouch calls from the same voucher (0 = disabled).
    pub vouch_cooldown_secs: u64,
    /// Minimum stake per vouch that yields non-zero yield (anti-dust guard).
    pub min_yield_stake: i128,
    /// Minimum age of a vouch before it can be used for loan eligibility, in seconds (default 24 hours).
    pub min_vouch_age_secs: u64,
    /// Prepayment penalty in basis points (e.g. 100 = 1%). Applied to remaining principal
    /// when a borrower repays early. 0 means no penalty.
    pub prepayment_penalty_bps: u32,
    /// #634: Liquidity mining reward rate in basis points per epoch (e.g. 50 = 0.5% per 7 days).
    pub liquidity_mining_rate_bps: u32,
    /// Voting period for slash-threshold governance proposals, in seconds.
    pub voting_period_seconds: u64,
    /// Minimum seconds between slashes for the same borrower (0 = disabled).
    pub slash_cooldown_seconds: u64,
        /// When true, critical write paths are blocked until multi-sig emergency unpause.
    pub emergency_pause_enabled: bool,
    /// Issue #668: Discount applied to yield on early repayment, in basis points (0 = no discount).
    pub early_repayment_discount_bps: u32,
    /// Issue #666/#667: Optional oracle contract address for repayment verification.
    pub oracle_address: Option<soroban_sdk::Address>,
    /// Delay (in seconds) after a slash vote reaches quorum before it can be executed (0 = immediate).
    pub slash_delay_seconds: u64,
    /// Designated successor admin address that can claim admin rights without multi-sig approval
    /// when current admins are unavailable.
    pub successor_admin: Option<Address>,
    pub rate_limit_config: RateLimitConfig,
    /// Issue #893: Multi-tier admin approval thresholds for different operation types.
    /// Empty means "not set" -- falls back to single admin_threshold for all operations.
    /// (0 or 1 elements; a `Vec` rather than `Option` because a custom struct nested in
    /// an `Option` field of a `#[contracttype]` isn't XDR-derivable in this SDK version.)
    pub multi_tier_thresholds: Vec<MultiTierAdminThresholds>,    /// Recovery percentage for defaulted loans (in basis points, e.g. 5000 = 50%).
    pub recovery_percentage: u32,
    /// When true, the slash threshold is calculated dynamically based on pool health.
    pub dynamic_slash_threshold: bool,
    /// When true, loan size affects the maximum slash basis points.
    pub loan_size_slash_enabled: bool,
    /// Maximum slash in basis points when loan size slash is enabled (e.g. 8000 = 80%).
    pub loan_size_slash_max_bps: i128,
    /// When true, loans require admin confirmation before being executed.
    pub confirmation_required: bool,
    /// Admin compensation rate in basis points (e.g. 100 = 1%).
    pub admin_compensation_bps: u32,
    /// Minimum votes required to remove an admin via governance (0 = disabled).
    pub removal_vote_threshold: u32,
    /// Insurance premium rate in basis points collected at loan disbursement (e.g. 100 = 1%).
    /// Controls where redistributable slash funds flow after insurance allocation.
    pub redistribution_rule: RedistributionRule,
    /// Seconds after repayment during which a borrower is immune from slash votes (0 = disabled).
    pub immunity_period_seconds: u64,
    pub insurance_premium_bps: u32,
    /// Issue #1077: Per-liquidity-tier yield bonus in basis points.
    /// Index 0 = Tier 0 (most liquid, no bonus), 3 = Tier 3 (illiquid, max bonus).
    /// Example: [0, 50, 150, 300] means tier-3 tokens earn +300 bps extra yield.
    pub liquidity_tier_yield_bonus: Vec<i128>,
    /// Issue #1072: Credit score decay rate per month in basis points (e.g. 100 = 1% per month).
    /// Applied monthly to encourage active participation and prevent stale scores.
    pub score_decay_per_month: u32,
    /// Issue #1287: Governance-adjustable cap on withdrawal-queue priority fees,
    /// in basis points of the voucher's own stake (default 1_000 = 10%).
    /// Replaces the compile-time constant `MAX_PRIORITY_FEE_BPS`.
    pub max_priority_fee_cap_bps: i128,
    /// Issue #1070: Default rate threshold (in basis points) that triggers circuit breaker.
    /// Default: 10_000 = 100 basis points = 10% of total loans defaulted.
    /// When `(default_count / total_loan_count) * 10_000 >= default_rate_threshold`,
    /// the circuit breaker automatically pauses the protocol.
    pub default_rate_threshold: u32,
    /// Issue #1071: Insurance fund configuration — premium percentage of loan principal
    /// to be collected and routed to the insurance pool (in basis points, e.g. 50 = 0.5%).
    pub insurance_fund_premium_bps: u32,
    /// Issue #1071: Maximum insurance payout as a percentage of total slashed amount
    /// (in basis points, e.g. 2500 = 25%).
    pub insurance_max_payout_bps: u32,
    /// Issue #10: Maximum number of consecutive refinances allowed in a single loan
    /// chain before the chain must be closed.  0 means no limit.
    /// Default: `DEFAULT_MAX_REFINANCES_PER_LOAN_CHAIN` (3).
    pub max_refinances_per_loan_chain: u32,
    /// Issue #10: Minimum time (in seconds) a borrower must wait between consecutive
    /// refinances.  0 means no cooldown.
    /// Default: `DEFAULT_REFINANCE_COOLDOWN_SECS` (7 days).
    pub refinance_cooldown_secs: u64,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct AmortizationEntry {
    pub due_date: u64,
    pub payment_due: i128,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    Pending,
    Submitted,
    Approved,
    Rejected,
    Expired,
}

#[contracttype]
#[derive(Clone)]
pub struct MilestoneRecord {
    pub milestone_id: u32,
    pub loan_id: u64,
    pub tranche_id: u32,
    pub status: MilestoneStatus,
    pub deadline: u64,
    pub description: soroban_sdk::String,
    pub submitted_at: Option<u64>,
    pub evidence_hash: Option<soroban_sdk::BytesN<32>>,
    pub proof_uri: Option<soroban_sdk::String>,
    pub approved_at: Option<u64>,
    pub approvers: Vec<Address>,
    pub rejection_reason: Option<soroban_sdk::String>,
    pub tranche_released: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct LoanRecord {
    pub id: u64,
    pub borrower: Address,
    pub guarantor: Option<Address>,
    pub buyback_price: i128,
    pub auto_repay_enabled: bool,
    pub auto_repay_attempts: u32,
    pub escrow_status: EscrowStatus,
    pub co_borrowers: Vec<Address>,
    /// Total loan principal disbursed, in stroops. 1 XLM = 10,000,000 stroops.
    pub amount: i128,
    /// Cumulative repayments received so far (principal + yield + interest), in stroops.
    pub amount_repaid: i128,
    /// Yield owed to vouchers, locked in at disbursement time, in stroops.
    /// Computed as `amount * yield_bps / 10_000`.
    pub total_yield: i128,
    pub status: LoanStatus,
    pub repaid: bool,
    pub defaulted: bool,
    /// Ledger timestamp when the loan record was created.
    pub created_at: u64,
    /// Ledger timestamp when the loan was disbursed to the borrower.
    pub disbursement_timestamp: u64,
    /// Ledger timestamp when the loan was fully repaid; `None` if not yet repaid.
    pub repayment_timestamp: Option<u64>,
    /// Repayment deadline as a ledger timestamp.
    pub deadline: u64,
    /// Borrower-supplied description of the loan purpose.
    pub loan_purpose: soroban_sdk::String,
    /// Address of the token contract used for this loan.
    pub token_address: Address,
    /// Amortization schedule for partial repayments.
    pub amortization_schedule: Vec<AmortizationEntry>,
    pub reminder_sent: bool,
    pub risk_score: u32,
    pub deferment_periods: u32,
    /// Optional custom maturity date (ledger timestamp).
    pub maturity_date: Option<u64>,
    pub rate_type: RateType,
    /// For variable-rate loans: the oracle key or index name.
    pub index_reference: Option<soroban_sdk::String>,
    // ── Daily-compound interest fields ───────────────────────────────────────
    /// Ledger timestamp of the last interest accrual.
    pub last_interest_calc: u64,
    /// Total compound interest accrued so far but not yet repaid.
    pub accrued_interest: i128,
    // ── Milestone bonus field ─────────────────────────────────────────────────
    /// Bitmask tracking which milestone bonuses have already been applied.
    /// Bit 0 = 25% milestone, bit 1 = 50% milestone, bit 2 = 75% milestone.
    pub milestone_bonus_applied: u32,
    /// Issue #669: Retry count for failed repayments (max 3).
    pub retry_count: u32,
    pub suspension_timestamp: Option<u64>,
    pub suspension_amount_repaid: i128,
}

/// An archived loan record, stored separately to reduce active persistent storage.
/// Created when a loan reaches a terminal state (Repaid or Defaulted) and is moved
/// from active storage to archive to preserve history while reducing bloat.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinanceRecord {
    pub old_loan_id: u64,
    pub new_loan_id: u64,
    pub borrower: Address,
    pub old_amount: i128,
    pub new_amount: i128,
    pub old_rate_bps: i128,
    pub new_rate_bps: i128,
    pub refinanced_at: u64,
}

/// Issue #1166: A non-binding quote for refinancing a borrower's active loan,
/// used for rate shopping before committing to `refinance_loan`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinanceQuote {
    pub borrower: Address,
    pub old_loan_id: u64,
    /// Outstanding balance (principal + yield - repaid) on the active loan.
    pub outstanding: i128,
    /// Effective rate, in basis points, of the current loan.
    pub old_rate_bps: i128,
    /// Effective rate the borrower would receive today, in basis points,
    /// accounting for their current credit tier.
    pub new_rate_bps: i128,
    /// Whether the borrower currently qualifies for a beneficial refinance
    /// (new_rate_bps < old_rate_bps and the loan has not passed its deadline).
    pub eligible: bool,
    /// Estimated interest cost saved over one year on the outstanding
    /// balance at the new rate vs. the old rate. Negative if the new rate
    /// is worse.
    pub estimated_annual_savings: i128,
    /// One-time protocol fee charged on the new loan amount, in stroops.
    pub refinance_fee: i128,
    /// Days of accrued savings needed to offset `refinance_fee`. `None` when
    /// the refinance produces no savings (fee is never recouped).
    pub breakeven_days: Option<u64>,
}

/// Issue #1166: Global aggregate statistics for refinance usage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinanceStats {
    pub total_refinances: u32,
    /// Sum of `old_rate_bps - new_rate_bps` (in bps) across all refinances,
    /// weighted by nothing — a simple running total for reporting.
    pub total_rate_reduction_bps: i128,
    /// Sum of estimated annual savings (in stroops) across all refinances,
    /// computed the same way as `RefinanceQuote::estimated_annual_savings`.
    pub total_estimated_savings: i128,
}

/// Issue #1167: One entry in a vouch's split genealogy — records that
/// `amount` was carved out of `parent_voucher`'s vouch and given to
/// `child_voucher` as a new, independent vouch for the same borrower.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VouchSplitRecord {
    pub parent_voucher: Address,
    pub child_voucher: Address,
    pub borrower: Address,
    pub amount: i128,
    pub split_at: u64,
}

/// Issue #1165: A vouch that has not rotated in a long time and is a
/// candidate for `rotate_to_new_borrower`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagnantVouch {
    pub voucher: Address,
    pub borrower: Address,
    pub stake: i128,
    pub days_since_rotation: u64,
}

/// Issue #1164: A single borrower's share of a voucher's total exposure.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowerExposure {
    pub borrower: Address,
    pub stake: i128,
    /// Share of the voucher's total stake, in basis points (10_000 = 100%).
    pub pct_bps: u32,
}

/// Issue #1164: A voucher's exposure to a single token, used as the
/// "sector" concentration axis (asset-class diversification).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenExposure {
    pub token: Address,
    pub stake: i128,
    pub pct_bps: u32,
}

/// Issue #1164: A voucher's exposure to a single chain, used as the
/// "region" concentration axis. `chain_id = None` means native Stellar.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainExposure {
    pub chain_id: Option<u32>,
    pub stake: i128,
    pub pct_bps: u32,
}

/// Issue #1164: A point-in-time snapshot of a voucher's portfolio, appended
/// to `DataKey::VoucherPortfolioHistory` whenever the risk report is read.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioSnapshot {
    pub timestamp: u64,
    pub total_stake: i128,
    pub borrower_count: u32,
}

/// Issue #1164: Full portfolio risk report for a voucher.
#[contracttype]
#[derive(Clone)]
pub struct PortfolioRiskReport {
    pub voucher: Address,
    pub total_stake: i128,
    pub borrower_count: u32,
    pub borrower_breakdown: Vec<BorrowerExposure>,
    pub token_breakdown: Vec<TokenExposure>,
    pub chain_breakdown: Vec<ChainExposure>,
    /// Herfindahl-Hirschman-style concentration index over borrower shares,
    /// in basis points (sum of pct_bps^2 / 10_000). Higher = more concentrated.
    pub concentration_hhi_bps: u32,
    /// Estimated loss if 1% of the voucher's backed borrowers default,
    /// weighted by stake (see `portfolio_risk` for the exact model).
    pub estimated_loss_1pct: i128,
    /// Estimated loss at a 5% default rate.
    pub estimated_loss_5pct: i128,
    /// Estimated loss at a 10% default rate.
    pub estimated_loss_10pct: i128,
    pub recommendations: Vec<soroban_sdk::String>,
    pub history: Vec<PortfolioSnapshot>,
}

#[contracttype]
#[derive(Clone)]
pub struct ArchivedLoanRecord {
    /// Unique archive ID (monotonically increasing).
    pub archive_id: u64,
    /// Original loan ID before archival.
    pub original_loan_id: u64,
    /// Borrower address for historical audit trail.
    pub borrower: Address,
    /// Total principal in stroops.
    pub amount: i128,
    /// Cumulative repayments in stroops.
    pub amount_repaid: i128,
    /// Total yield locked in stroops.
    pub total_yield: i128,
    /// Final loan status before archival (should be Repaid or Defaulted).
    pub final_status: LoanStatus,
    /// Timestamp when the loan was originally created.
    pub created_at: u64,
    /// Timestamp when the loan was archived (terminal state reached).
    pub archived_at: u64,
    /// Original loan purpose for audit trail.
    pub loan_purpose: soroban_sdk::String,
    /// Token used for this loan.
    pub token_address: Address,
}

/// Issue #1172: Guarantor record for a loan.
/// Tracks the guarantor backing a loan and their obligations.
#[contracttype]
#[derive(Clone)]
pub struct GuarantorRecord {
    /// Loan ID this guarantor is backing
    pub loan_id: u64,
    /// Guarantor address
    pub guarantor: Address,
    /// Guarantor signature commitment (to verify backing)
    pub signature_verified: bool,
    /// Amount guaranteed (in stroops) — can be less than full loan amount
    pub guarantee_amount: i128,
    /// Token this guarantee's stake is denominated and locked in (#1406).
    /// Recorded once at `request_guarantor_for_loan` time and authoritative for
    /// the entire lifetime of the guarantee — `claim_guarantor_coverage` pays
    /// out in this token rather than trusting a caller-supplied token address.
    pub token: Address,
    /// Timestamp when guarantor was requested for this loan
    pub requested_at: u64,
    /// Timestamp when guarantor was released (None if still active)
    pub released_at: Option<u64>,
    /// Status of the guarantee
    pub status: GuaranteeStatus,
}

/// Status of a guarantee.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuaranteeStatus {
    /// Guarantee is active and binding
    Active,
    /// Guarantee has been released after loan completion
    Released,
    /// Guarantee has been triggered (borrower defaulted)
    Triggered,
    /// Guarantee has been claimed (funds distributed)
    Claimed,
}

/// Issue #1172: Guarantor obligation tracking.
/// Tracks what the guarantor owes if the borrower defaults.
#[contracttype]
#[derive(Clone)]
pub struct GuarantorObligation {
    /// Guarantor address
    pub guarantor: Address,
    /// Loan ID
    pub loan_id: u64,
    /// Borrower address
    pub borrower: Address,
    /// Maximum amount guarantor is liable for (in stroops)
    pub max_liability: i128,
    /// Amount already paid by guarantor (in stroops)
    pub amount_paid: i128,
    /// Timestamp when obligation was created
    pub created_at: u64,
    /// Timestamp when obligation was fulfilled or waived
    pub closed_at: Option<u64>,
}

/// Issue #1172: Guarantor reputation and statistics.
#[contracttype]
#[derive(Clone)]
pub struct GuarantorStats {
    /// Total number of guarantees provided
    pub total_guarantees: u32,
    /// Number of successfully fulfilled guarantees
    pub successful_guarantees: u32,
    /// Number of triggered guarantees (defaults)
    pub triggered_guarantees: u32,
    /// Total amount guaranteed across all loans (in stroops)
    pub total_guaranteed: i128,
    /// Total amount paid out on triggered guarantees (in stroops)
    pub total_paid_out: i128,
    /// Reputation score (0-1000): higher = better guarantor
    pub reputation_score: u32,
    /// Last active timestamp
    pub last_activity: u64,
}

/// Issue #1175: Vouch slashing protection bond.
/// Bonds limit the maximum loss a voucher can suffer if a borrower defaults.
#[contracttype]
#[derive(Clone)]
pub struct VouchProtectionBond {
    /// Voucher address
    pub voucher: Address,
    /// Loan ID this bond is protecting
    pub loan_id: u64,
    /// Vouch ID (typically matches loan_id in current design)
    pub vouch_id: u64,
    /// Bond amount staked (in stroops) - covers up to 50% of vouch amount
    pub bond_amount: i128,
    /// The vouch stake this bond is protecting
    pub protected_stake: i128,
    /// Timestamp when bond was created
    pub created_at: u64,
    /// Amount of bond used to cover slash (in stroops)
    pub amount_used: i128,
    /// Timestamp when bond was released (None if still active)
    pub released_at: Option<u64>,
    /// Status of the bond
    pub status: BondStatus,
    /// Whether optional bond insurance was purchased
    pub has_insurance: bool,
}

/// Status of a vouch protection bond.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BondStatus {
    /// Bond is active and protecting the vouch
    Active,
    /// Bond has been partially used to cover a slash
    PartiallyUsed,
    /// Bond has been fully used to cover a slash
    Exhausted,
    /// Bond has been released after loan completion
    Released,
}

/// Issue #1175: Optional bond insurance.
/// Provides additional coverage for the bond with a 3% premium surcharge.
#[contracttype]
#[derive(Clone)]
pub struct BondInsuranceRecord {
    /// Voucher address
    pub voucher: Address,
    /// Loan ID
    pub loan_id: u64,
    /// Bond amount covered by insurance
    pub insured_bond_amount: i128,
    /// Insurance premium paid (3% of bond amount)
    pub premium_paid: i128,
    /// Maximum payout (typically 100% of bond amount)
    pub max_coverage: i128,
    /// Amount claimed under insurance (if any)
    pub amount_claimed: i128,
    /// Status of the insurance
    pub status: InsuranceStatus,
    /// Timestamp when insurance was purchased
    pub purchased_at: u64,
}

/// Status of bond insurance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InsuranceStatus {
    /// Insurance is active
    Active,
    /// Insurance claim has been paid
    Claimed,
    /// Insurance has been cancelled/released
    Released,
}

/// Issue #1175: Bond tracking and statistics.
#[contracttype]
#[derive(Clone)]
pub struct BondStats {
    /// Voucher address
    pub voucher: Address,
    /// Total bond amount across all loans (in stroops)
    pub total_bonded: i128,
    /// Total bond amount used to cover slashes (in stroops)
    pub total_used: i128,
    /// Number of active bonds
    pub active_bonds: u32,
    /// Number of times this voucher's bond was used
    pub times_bond_used: u32,
    /// Total bond insurance premiums paid (in stroops)
    pub total_insurance_premiums: i128,
    /// Number of insurance claims paid
    pub insurance_claims_paid: u32,
    /// Total insurance payout (in stroops)
    pub total_insurance_payout: i128,
    /// Last activity timestamp
    pub last_activity: u64,
}

/// A reference to archived data stored on IPFS.
/// The actual data blob is stored on IPFS, and this contract maintains the hash for retrieval.
#[contracttype]
#[derive(Clone)]
pub struct IpfsArchiveReference {
    /// IPFS content hash (e.g., "Qm..." for v0 IPFS, "baf..." for v1 CIDv1)
    pub ipfs_hash: soroban_sdk::String,
    /// Timestamp when this archive was created
    pub archived_at: u64,
    /// Type of archive: "loan", "vouch_history", etc.
    pub archive_type: soroban_sdk::String,
}

/// #645: Pending loan restructure request — borrower requests, vouchers approve.
#[contracttype]
#[derive(Clone)]
pub struct RestructureRequest {
    pub borrower: Address,
    /// New deadline (must be after current deadline).
    pub new_deadline: u64,
    /// Reduced outstanding amount (0 = no change to amount).
    pub new_amount: i128,
    /// Ledger timestamp when the request was created.
    pub requested_at: u64,
    /// Voucher addresses that have approved this request.
    pub approvals: Vec<Address>,
}

/// A single payment event recorded against a loan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRecord {
    /// Amount paid in this transaction, in stroops.
    pub amount: i128,
    /// Ledger timestamp of this payment.
    pub timestamp: u64,
    /// Cumulative amount repaid after this payment, in stroops.
    pub cumulative_repaid: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct VouchRecord {
    pub voucher: Address,
    /// Amount staked by the voucher, in stroops. 1 XLM = 10,000,000 stroops.
    pub stake: i128,
    /// Ledger timestamp when this vouch was created; immutable after set.
    pub vouch_timestamp: u64,
    /// Token contract address that this stake is denominated in.
    pub token: Address,
    /// Optional expiry timestamp; if set and current time > expiry, vouch is expired.
    pub expiry_timestamp: Option<u64>,
    /// Optional delegate address; if set, this address can manage the vouch.
    pub delegate: Option<Address>,
    /// Optional chain ID for cross-chain vouches. `None` means native Stellar.
    /// When set, the token must originate from a registered bridge for that chain.
    pub chain_id: Option<u32>,
}

/// Issue #1173: Vouch reputation weighted strength.
/// Tracks the reputation-adjusted strength of a vouch in quorum calculations.
#[contracttype]
#[derive(Clone)]
pub struct VouchReputationWeight {
    /// Identifies the vouch this record belongs to — see
    /// `vouch_reputation::derive_vouch_id` (#1408) for how it's derived.
    pub vouch_id: u64,
    /// Base strength of the vouch (the raw stake)
    pub base_strength: i128,
    /// Voucher's reputation score (0-1000)
    pub voucher_reputation: u32,
    /// Calculated weighted strength: base_strength × (1 + (reputation / 1000))
    /// Capped at 1.5x multiplier for reputation >= 1500
    pub weighted_strength: i128,
    /// Weight multiplier applied (in basis points, e.g., 1000 = 1.0x, 1500 = 1.5x)
    pub weight_multiplier_bps: u32,
    /// Timestamp when weight was last calculated
    pub calculated_at: u64,
}

/// Issue #1173: Weighted vouch distribution for a borrower.
/// Tracks aggregate reputation-weighted vouch strength for quorum calculations.
#[contracttype]
#[derive(Clone)]
pub struct WeightedVouchDistribution {
    /// Borrower address
    pub borrower: Address,
    /// Token address
    pub token: Address,
    /// Total base stake (unweighted)
    pub total_base_stake: i128,
    /// Total weighted stake (reputation-adjusted)
    pub total_weighted_stake: i128,
    /// Number of vouches contributing
    pub vouch_count: u32,
    /// Average weight multiplier across all vouches (in basis points)
    pub average_weight_multiplier_bps: u32,
    /// Timestamp when distribution was last updated
    pub updated_at: u64,
}

/// Metadata for a registered cross-chain bridge.
#[contracttype]
#[derive(Clone)]
pub struct BridgeRecord {
    /// Numeric chain identifier (e.g. 1 = Ethereum mainnet, 137 = Polygon).
    pub chain_id: u32,
    /// Human-readable chain name (e.g. "ethereum", "polygon").
    pub chain_name: soroban_sdk::String,
    /// The Stellar-side bridge contract address that wraps/unwraps tokens.
    pub bridge_address: Address,
    /// Whether this bridge is currently active and accepted for new vouches.
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct VouchHistoryEntry {
    /// Timestamp of the modification.
    pub timestamp: u64,
    /// Type of modification: "created", "increased", "decreased", "withdrawn", "delegated".
    pub modification_type: soroban_sdk::String,
    /// Stake amount involved in the modification, in stroops.
    pub stake_amount: i128,
    /// Optional delegate address if this is a delegation event.
    pub delegate: Option<Address>,
}

/// Issue #1179: kind of event recorded in a vouch's audit trail.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VouchAuditEventType {
    /// The vouch was first created.
    Created,
    /// The voucher increased their stake on an existing vouch.
    StakeIncreased,
    /// The voucher decreased their stake on an existing vouch.
    StakeDecreased,
    /// The vouch was fully withdrawn.
    Withdrawn,
}

/// Issue #1179: a single immutable audit-trail entry for a (borrower,
/// voucher, token) vouch relationship, suitable for compliance and
/// transparency reporting.
#[contracttype]
#[derive(Clone)]
pub struct VouchAuditEvent {
    /// Kind of event this entry records.
    pub event_type: VouchAuditEventType,
    /// Ledger timestamp at which the event occurred.
    pub timestamp: u64,
    /// Amount involved in the event: the stake for `Created`, the delta for
    /// `StakeIncreased`/`StakeDecreased`, and the returned stake for `Withdrawn`.
    pub amount: i128,
    /// The vouch's total stake immediately after this event (0 after `Withdrawn`).
    pub resulting_stake: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct LoanPoolRecord {
    pub pool_id: u64,
    pub borrowers: Vec<Address>,
    /// Per-borrower loan amounts in this pool, in stroops. 1 XLM = 10,000,000 stroops.
    pub amounts: Vec<i128>,
    /// Ledger timestamp when this pool was created.
    pub created_at: u64,
    /// Total amount disbursed from this pool across all borrowers, in stroops.
    /// 1 XLM = 10,000,000 stroops.
    pub total_disbursed: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct TimelockProposal {
    pub id: u64,
    pub action: TimelockAction,
    pub proposer: Address,
    pub eta: u64,
    pub executed: bool,
    pub cancelled: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum TimelockAction {
    Slash(Address),
    SetConfig(Config),
}

/// A pending slash awaiting execution after the mandatory delay period.
/// Created when a slash vote reaches quorum; executed via `execute_pending_slash`.
#[contracttype]
#[derive(Clone)]
pub struct PendingSlashRecord {
    pub borrower: Address,
    pub approved_at: u64,
    pub executable_at: u64,
    pub executed: bool,
}

/// A queued slash entry for lazy/deferred batch execution.
/// Created via `queue_slash`; executed via `execute_queued_slashes`.
#[contracttype]
#[derive(Clone)]
pub struct LazySlashEntry {
    pub borrower: Address,
    pub amount: i128,
    pub queued_at: u64,
}

/// Controls where redistributable slash funds flow after insurance allocation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedistributionRule {
    /// Route to the slash treasury (default).
    Treasury,
    /// Redistribute pro-rata to remaining active vouchers of the borrower.
    Vouchers,
}

#[contracttype]
#[derive(Clone)]
pub struct SlashRecord {
    pub slash_id: u64,
    pub borrower: Address,
    pub loan_id: u64,
    pub loan_amount: i128,
    pub total_slashed: i128,
    pub slash_timestamp: u64,
    /// Amount returned to borrower from treasury on full repay (0 until recovered).
    pub recovery_amount: i128,
    /// Set by admin on reversal; None when not reversed.
    pub reversal_reason: Option<soroban_sdk::String>,
    /// True once an admin has reversed this slash.
    pub reversed: bool,
    /// Effective slash percentage (basis points) applied at slash time.
    /// Used to correctly restore funds on successful appeal.
    pub effective_slash_bps: i128,
}

/// Monthly aggregated report of all slashing events.
#[contracttype]
#[derive(Clone)]
pub struct SlashingReportRecord {
    /// Month identifier: unix_timestamp / MONTHLY_PERIOD_SECS.
    pub month_id: u64,
    /// Total number of slash events in this month.
    pub total_slashes: u32,
    /// Total amount slashed across all events, in stroops.
    pub total_slashed: i128,
    /// Number of slashes subsequently reversed by admins.
    pub total_reversed: u32,
    /// Slash IDs recorded during this month.
    pub slash_ids: Vec<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRequest {
    pub voucher: Address,
    pub borrower: Address,
    pub token: Address,
    pub requested_at: u64,
}

/// A queued withdrawal request submitted during an active loan.
/// Processed automatically when the loan is repaid or slashed.
#[contracttype]
#[derive(Clone)]
pub struct QueuedWithdrawal {
    /// The voucher requesting withdrawal.
    pub voucher: Address,
    /// Token the stake is denominated in.
    pub token: Address,
    /// Ledger timestamp when the request was submitted.
    pub requested_at: u64,
    /// Whether this is a partial withdrawal (up to 50% of stake with penalty).
    pub partial: bool,
    /// Priority fee paid by the voucher (in stroops), distributed to remaining vouchers.
    pub priority_fee: i128,
}

/// Per-vouch yield allocation, locked at loan disbursement.
#[contracttype]
#[derive(Clone)]
pub struct YieldDistributionEntry {
    pub voucher: Address,
    pub yield_amount: i128,
}

/// Cumulative vouch reputation statistics for a voucher address.
#[contracttype]
#[derive(Clone)]
pub struct VoucherStats {
    pub successful_vouches: u32,
    pub total_vouches_slashed: u32,
    pub total_yield_earned: i128,
    pub total_slashed: i128,
}

/// Maximum number of loan entries to keep in the LRU cache.
pub const CACHE_LRU_MAX_ENTRIES: u32 = 100;
/// TTL for general cached records, in seconds (5 minutes).
pub const CACHE_TTL_SECS: u64 = 5 * 60;
/// TTL for cached yield-bps values, in seconds (5 minutes).
pub const YIELD_CACHE_TTL_SECS: u64 = 5 * 60;

/// Cached per-vouch yield rate for a (borrower, voucher) pair.
#[contracttype]
#[derive(Clone)]
pub struct CachedYieldRecord {
    /// The cached yield rate in basis points.
    pub yield_bps: i128,
    /// Ledger timestamp when this value was cached.
    pub cached_at: u64,
    /// The base yield_bps from config at cache time (for stale-config detection).
    pub base_yield_bps: i128,
}

/// Current API version of the contract.
pub const API_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

// ── Contract Versioning (Issue #742) ─────────────────────────────────────────

/// Semantic version record stored on-chain for the contract itself.
#[contracttype]
#[derive(Clone)]
pub struct ContractSemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// Ledger timestamp when this version was set.
    pub updated_at: u64,
    /// Short human-readable change note (max 64 chars).
    pub note: soroban_sdk::String,
}

/// A single entry in the on-chain version history log.
#[contracttype]
#[derive(Clone)]
pub struct VersionHistoryEntry {
    pub version: ContractSemVer,
    /// Sequential index of this entry (0-based).
    pub index: u32,
}

// ── Deployment Records (Issue #743) ──────────────────────────────────────────

/// On-chain record of a single contract deployment or upgrade.
#[contracttype]
#[derive(Clone)]
pub struct DeploymentRecord {
    /// Sequential deployment index (0-based).
    pub index: u32,
    /// Deployer address that signed the transaction.
    pub deployer: Address,
    /// Ledger timestamp of the deployment.
    pub deployed_at: u64,
    /// Semantic version active at time of deployment.
    pub version: ContractSemVer,
    /// Network identifier ("testnet" | "mainnet").
    pub network: soroban_sdk::String,
}

// ── Rollback Snapshots (Issue #744) ──────────────────────────────────────────

/// Snapshot of critical config fields saved before an upgrade, used for rollback.
#[contracttype]
#[derive(Clone)]
pub struct RollbackSnapshot {
    /// Deployment index this snapshot corresponds to.
    pub deployment_index: u32,
    /// Ledger timestamp when the snapshot was taken.
    pub snapshot_at: u64,
    /// Semantic version at snapshot time.
    pub version: ContractSemVer,
    /// Serialised config — stores yield_bps, slash_bps, max_vouchers, and
    /// admin_threshold so a rollback can restore these critical parameters.
    pub yield_bps: i128,
    pub slash_bps: i128,
    pub max_vouchers: u32,
    pub admin_threshold: u32,
}

// ── API Caching (Issue #724) ──────────────────────────────────────────────────

/// Issue #687: Governance proposal to remove a compromised admin address.
/// Passes when `approve_votes >= Config.removal_vote_threshold`.
#[contracttype]
#[derive(Clone)]
pub struct AdminRemovalProposal {
    pub id: u64,
    /// Admin address to be removed if the proposal passes.
    pub admin_to_remove: Address,
    /// Address that created the proposal (must be a governance participant).
    pub proposer: Address,
    /// Number of approve votes cast so far.
    pub approve_votes: u32,
    /// Number of reject votes cast so far.
    pub reject_votes: u32,
    /// Addresses that have already voted (prevent double-voting).
    pub voters: Vec<Address>,
    /// Ledger timestamp when the proposal was created.
    pub proposed_at: u64,
    /// True once the proposal has been finalized (admin removed or rejected).
    pub finalized: bool,
}

// ── Pagination ────────────────────────────────────────────────────────────────

#[contracttype]
pub enum CacheKey {
    LoanCache(u64),           // loan_id → CachedLoanRecord
    VouchesCache(Address),    // borrower → CachedVouchesRecord
    ConfigCache,              // CachedConfigRecord
}

#[contracttype]
#[derive(Clone)]
pub struct CachedLoanRecord {
    pub data: LoanRecord,
    pub cached_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CachedVouchesRecord {
    pub data: Vec<VouchRecord>,
    pub cached_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CachedConfigRecord {
    pub data: Config,
    pub cached_at: u64,
}

// ── Risk Assessment Voting (Issue #903) ──────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct RiskThresholdProposal {
    pub id: u64,
    pub proposer: Address,
    pub min_risk_threshold: u32,  // basis points (e.g., 5000 = 50%)
    pub max_risk_threshold: u32,  // basis points
    pub votes_for: i128,
    pub votes_against: i128,
    pub status: GovernanceProposalStatus,
    pub created_at: u64,
    pub eta: u64,
}

// ── Fee Structure Voting (Issue #904) ──────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct FeeStructureProposal {
    pub id: u64,
    pub proposer: Address,
    pub origination_fee_bps: u32,
    pub repayment_fee_bps: u32,
    pub late_fee_bps: u32,
    pub votes_for: i128,
    pub votes_against: i128,
    pub status: GovernanceProposalStatus,
    pub created_at: u64,
    pub eta: u64,
}

// ── Withdrawal Timelock (Issue #905) ───────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct WithdrawalTimelock {
    pub id: u64,
    pub voucher: Address,
    pub borrower: Address,
    pub amount: i128,
    pub token: Address,
    pub eta: u64,
    pub executed: bool,
    pub cancelled: bool,
}

// ── Cross-Chain Proposal Sync (Issue #906) ────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct CrossChainProposalSync {
    pub id: u64,
    pub source_chain: String,
    pub target_chains: Vec<String>,
    pub proposal_type: String,  // "risk", "fee", "timelock"
    pub proposal_data: soroban_sdk::Bytes,
    pub votes_required: u32,
    pub votes_received: u32,
    pub status: GovernanceProposalStatus,
    pub created_at: u64,
    pub eta: u64,
}

// ── Error Standardization (Issue #725) ────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct ErrorResponse {
    /// Numeric error code matching ContractError enum.
    pub code: u32,
    /// Human-readable error message.
    pub message: soroban_sdk::String,
    /// Optional additional context or details.
    pub details: Option<soroban_sdk::String>,
    /// Timestamp when the error occurred.
    pub timestamp: u64,
}

// ── Issue #868: Gradual Unstaking ────────────────────────────────────────────

/// Default number of equal instalments for gradual unstaking (4 tranches).
pub const DEFAULT_GRADUAL_UNSTAKE_INSTALMENTS: u32 = 4;
/// Default interval between instalments, in seconds (7 days).
pub const DEFAULT_GRADUAL_UNSTAKE_INTERVAL_SECS: u64 = 7 * 24 * 60 * 60;

/// Progressive vouch-revocation schedule: stake released in equal instalments.
#[contracttype]
#[derive(Clone)]
pub struct GradualUnstakeSchedule {
    pub voucher: Address,
    pub borrower: Address,
    pub token: Address,
    /// Total stake to release across all instalments, in stroops.
    pub total_amount: i128,
    /// Amount per instalment, in stroops.
    pub instalment_amount: i128,
    pub instalments_paid: u32,
    pub total_instalments: u32,
    pub interval_secs: u64,
    pub created_at: u64,
    /// Ledger timestamp when the next instalment becomes claimable.
    pub next_release_at: u64,
}

// ── Issue #884: Prepayment Bonus ────────────────────────────────────────────

/// Default prepayment bonus rate in basis points (50 = 0.5% of loan amount).
pub const DEFAULT_PREPAYMENT_BONUS_BPS: u32 = 50;

// ── Issue #885: Loan Status Privacy ─────────────────────────────────────────

/// Privacy level for loan status visibility.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanPrivacyLevel {
    /// Anyone can view loan details (default).
    Public,
    /// Only the borrower and their vouchers can view loan details.
    VouchersOnly,
    /// Only the borrower can view loan details.
    Private,
}

// ── Issue #887: Loan Subordination and Cascading Debt Hierarchy ──────────────

/// Issue #887: Subordination level in the debt hierarchy.
/// Determines priority order for repayment and default cascading.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SubordinationLevel {
    /// Senior (Priority 0): Highest priority. Must be fully repaid first.
    /// Default of senior loan blocks all subordinate loans.
    Senior = 0,
    /// Mezzanine (Priority 1): Intermediate level.
    /// Can have both senior and subordinate loans.
    Mezzanine = 1,
    /// Subordinate (Priority 2+): Lowest priority.
    /// Repaid after seniors. Affected by senior defaults (cascading).
    Subordinate = 2,
}

/// Issue #887: Represents a subordination relationship between two loans.
/// Links a subordinate (junior) loan to its senior (creditor priority) loan.
#[contracttype]
#[derive(Clone)]
pub struct SubordinationRecord {
    /// ID of the senior (higher priority) loan
    pub senior_loan_id: u64,
    /// ID of the subordinate (lower priority) loan
    pub subordinate_loan_id: u64,
    /// The subordination level relative to the senior loan
    pub subordination_level: SubordinationLevel,
    /// Ledger timestamp when this subordination relationship was created
    pub created_at: u64,
    /// Whether this subordination is currently active (true) or waived (false)
    pub is_active: bool,
    /// Priority order index if senior loan has multiple subordinates (0 = highest priority)
    pub priority_index: u32,
}

/// Issue #887: Represents cascading default information.
/// Tracks which loans are affected when a senior loan defaults.
#[contracttype]
#[derive(Clone)]
pub struct CascadingDefault {
    /// ID of the senior loan that defaulted and triggered the cascade
    pub triggering_senior_loan_id: u64,
    /// IDs of all subordinate loans affected by this default
    pub affected_subordinate_ids: Vec<u64>,
    /// Ledger timestamp when the cascade was triggered
    pub triggered_at: u64,
    /// Whether the cascade has been fully resolved (all affected loans handled)
    pub is_resolved: bool,
}

/// Issue #887: Waterfall repayment distribution result.
/// Specifies how a repayment should be split between senior and subordinate loans.
#[contracttype]
#[derive(Clone)]
pub struct WaterfallDistribution {
    /// Amount to apply to the senior loan in stroops
    pub senior_amount: i128,
    /// Amount to apply to subordinate loans in stroops
    pub subordinate_amount: i128,
    /// Total amount distributed across all tiers
    pub total_distributed: i128,
}

/// Issue #887: DataKey for subordination relationships
/// Added to DataKey enum for storage:
/// `SubordinationRelation(u64, u64)` => (senior_loan_id, subordinate_loan_id) -> SubordinationRecord
/// `SubordinateLoansList(u64)` => senior_loan_id -> Vec<u64> (IDs of all subordinate loans)
/// `SeniorLoanOf(u64)` => subordinate_loan_id -> u64 (ID of direct senior loan)
/// `CascadingDefaultRecord(u64)` => senior_loan_id -> CascadingDefault
pub const MAX_SUBORDINATION_DEPTH: u32 = 10; // Prevent deeply nested hierarchies
pub const MAX_SUBORDINATES_PER_LOAN: u32 = 50; // Prevent excessive branching

/// Result for a single entry in `batch_vouch` with selective rollback semantics (Issue #1055).
/// Successful entries are committed; failed entries are skipped with an error code.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchVouchResult {
    /// The borrower address for this entry.
    pub borrower: Address,
    /// The stake amount attempted for this entry.
    pub stake: i128,
    /// `true` if the vouch was committed successfully; `false` if it was skipped.
    pub success: bool,
    /// Error code if `success == false`; `None` when successful.
    pub error_code: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchLoanStatusResult {
    pub borrower: Address,
    pub status: LoanStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyRecord {
    pub key: String,
    pub response_hash: BytesN<32>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteSlashResult {
    VoteCounted,
    DelegateWillVote,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ForbearanceStatus {
    Active,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForbearanceRecord {
    pub loan_id: u64,
    pub borrower: Address,
    pub started_at: u64,
    pub duration_secs: u64,
    pub ends_at: u64,
    pub original_deadline: u64,
    pub period_number: u32,
    pub status: ForbearanceStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowerDynamicRate {
    pub borrower: Address,
    pub loan_id: u64,
    pub effective_rate_bps: u32,
    pub risk_score: u32,
    pub credit_tier: CreditTier,
    pub computed_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicRateConfig {
    pub enabled: bool,
    pub base_rate_bps: u32,
    pub risk_adjustment_bps: u32,
    pub rate_floor_bps: u32,
    pub rate_cap_bps: u32,
    /// Oracle price key to consult as a risk input.
    pub oracle_price_symbol: Option<soroban_sdk::Symbol>,
    /// Price threshold below which oracle risk premium is applied.
    pub oracle_risk_threshold: i128,
    /// Risk premium applied when oracle price is below threshold.
    pub oracle_risk_premium_bps: u32,
    /// Conservative premium applied when oracle price is missing or stale.
    pub oracle_stale_premium_bps: u32,
}

pub const DEFAULT_DYNAMIC_RATE_CONFIG: DynamicRateConfig = DynamicRateConfig {
    enabled: false,
    base_rate_bps: 1000,
    risk_adjustment_bps: 10,
    rate_floor_bps: 500,
    rate_cap_bps: 2000,
    oracle_price_symbol: None,
    oracle_risk_threshold: 0,
    oracle_risk_premium_bps: 0,
    oracle_stale_premium_bps: 0,
};

pub const DEFAULT_FORBEARANCE_DURATION_SECS: u64 = 30 * 24 * 60 * 60;
pub const MAX_FORBEARANCE_PERIODS: u32 = 3;

// ── Oracle Price ──────────────────────────────────────────────────────────────

/// Staleness window for oracle price records, in seconds (1 hour).
pub const ORACLE_PRICE_MAX_AGE_SECS: u64 = 60 * 60;

/// An oracle price record with a value and timestamp.
#[contracttype]
#[derive(Clone)]
pub struct OraclePriceRecord {
    pub price: i128,
    pub recorded_at: u64,
}

// ── Graduated Response / Tiered Lockdown ──────────────────────────────────────

/// Protocol threat level for graduated response.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ThreatLevel {
    Normal,
    Elevated,
    Critical,
    Lockdown,
}

// ── Vouch Merkle Root ─────────────────────────────────────────────────────────

/// Vouch Merkle root record stored per borrower.
#[contracttype]
#[derive(Clone)]
pub struct VouchMerkleRoot {
    pub root: BytesN<32>,
    pub vouch_count: u32,
    pub computed_at: u64,
}

/// Issue #1056/#1372: emergency governance-voted waiver of the vouch cooldown.
/// See docs/vouch-cooldown-bypass-1056.md for the full design.
#[contracttype]
#[derive(Clone)]
pub struct CooldownBypassRequest {
    pub voucher: Address,
    pub borrower: Address,
    pub reason: String,
    pub requested_at: u64,
    pub approvers: Vec<Address>,
    pub approved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminActionProposal {
    pub id: u64,
    pub action_type: GovernanceAction,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashAppealRecord {
    pub borrower: Address,
    pub voucher: Address,
    pub evidence_hash: BytesN<32>,
    pub appeal_timestamp: u64,
    pub approved: Option<bool>,
    pub admin_votes: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FraudScoreConfig {
    pub threshold: u32,
    pub enabled: bool,
}

/// Issue #1424: a single historical circuit-breaker activation, retained in the
/// bounded `DataKey::CircuitBreakerHistory` log so operators can audit how often
/// the breaker has fired and correlate incidents with default-rate spikes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerTrigger {
    /// Ledger timestamp at which the breaker activated.
    pub timestamp: u64,
    /// Protocol-wide defaulted-loan count at activation time.
    pub default_count: u32,
    /// Protocol-wide total-loan count at activation time.
    pub total_loan_count: u32,
    /// Default rate in basis points at activation time.
    pub rate_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainLoanMetadata {
    pub origin_chain: u32,
    pub loan_id: u64,
    pub borrower: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeAttestation {
    pub signature: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnifiedReputation {
    pub score: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VouchGroup {
    pub group_id: u64,
    pub name: soroban_sdk::String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoucherYieldClaim {
    pub claimed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldStreamState {
    pub last_claim: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeriodicPaymentConfig {
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeriodicPaymentStatus {
    pub last_payment: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayEvent {
    pub source_chain: u32,
    pub dest_chain: u32,
    pub event_type: soroban_sdk::Symbol,
    pub payload: Bytes,
    pub seq: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayAttestation {
    pub signature: BytesN<64>,
    pub nonce: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeEntry {
    pub key: soroban_sdk::String,
    pub value: soroban_sdk::String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoucherFraudScore {
    pub score: u32,
}

/// Issue #1193: Loan covenant monitoring types
/// Covenants are financial and operational requirements that borrowers must maintain
/// throughout the loan lifecycle. Violations trigger escalation protocols.
/// Covenant type enumeration for different monitoring requirements
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CovenantType {
    /// Loan-to-value ratio covenant: Loan amount ≤ LTV% of collateral
    LoanToValue,
    /// Debt-to-income ratio: Total debt ≤ DTI% of borrower income
    DebtToIncome,
    /// Minimum payment schedule: Payments on time each period
    PaymentSchedule,
    /// Activity requirement: Minimum transaction volume per period
    ActivityRequirement,
    /// Collateral maintenance: Collateral value must not fall below threshold
    CollateralMaintenance,
    /// Cross-default: Triggered by defaults on other platforms
    CrossDefault,
}

/// Covenant breach severity levels for escalation
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreachSeverity {
    /// Minor breach: Warning stage
    Warning,
    /// Moderate breach: Review required
    Moderate,
    /// Critical breach: Immediate action required
    Critical,
}

/// Escalation stage in the covenant monitoring process
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EscalationStage {
    /// Initial warning notification
    Warning,
    /// Active review process
    UnderReview,
    /// Preparation for acceleration
    PendingAcceleration,
    /// Loan acceleration triggered
    Accelerated,
}

/// Configuration for a loan's covenants
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanCovenantConfig {
    /// Loan ID this config applies to
    pub loan_id: u64,
    /// Types of covenants active for this loan
    pub covenant_types: Vec<CovenantType>,
    /// LTV ratio in basis points (e.g., 8000 = 80%)
    pub ltv_ratio_bps: u32,
    /// DTI ratio in basis points (e.g., 4500 = 45%)
    pub dti_ratio_bps: u32,
    /// Minimum activity required (transactions per period)
    pub min_activity_per_period: u32,
    /// Collateral maintenance threshold in basis points
    pub collateral_maintenance_bps: u32,
    /// Monitoring period in seconds
    pub monitoring_period_secs: u64,
    /// Number of breaches allowed before escalation
    pub breach_tolerance: u32,
}

/// Current compliance status of a loan's covenants
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanCovenantStatus {
    /// Loan ID being monitored
    pub loan_id: u64,
    /// Current escalation stage
    pub escalation_stage: EscalationStage,
    /// Number of recorded breaches
    pub breach_count: u32,
    /// Timestamp of most recent breach
    pub last_breach_timestamp: u64,
    /// Timestamp of last monitoring check
    pub last_check_timestamp: u64,
    /// Whether covenant acceleration has been triggered
    pub is_accelerated: bool,
    /// Timestamp of acceleration (if triggered)
    pub acceleration_timestamp: u64,
}

/// Individual covenant breach record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CovenantBreach {
    /// Loan ID with breach
    pub loan_id: u64,
    /// Type of covenant violated
    pub covenant_type: CovenantType,
    /// Severity of the breach
    pub severity: BreachSeverity,
    /// Breach detection timestamp
    pub detected_timestamp: u64,
    /// Description of the breach (e.g., "LTV 92% exceeds 80% limit")
    pub description: soroban_sdk::String,
    /// Value that triggered the breach
    pub violation_value: i128,
    /// Allowed threshold value
    pub threshold_value: i128,
    /// Whether this breach triggered escalation
    pub triggered_escalation: bool,
}

/// Covenant monitoring event record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CovenantMonitoringEvent {
    /// Loan ID being monitored
    pub loan_id: u64,
    /// Event timestamp
    pub event_timestamp: u64,
    /// Event type description
    pub event_type: soroban_sdk::String,
    /// Previous escalation stage
    pub previous_stage: EscalationStage,
    /// New escalation stage
    pub new_stage: EscalationStage,
    /// Additional context about the event
    pub details: soroban_sdk::String,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConfigField {
    Dummy,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPatch {
    pub field: ConfigField,
    pub new_value: i128,
    pub apply_after: u64,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScheduleType {
    Dummy,
}

// ── Issue #1171: Vouch syndication ────────────────────────────────────────────

/// A single voucher's contribution when creating or joining a syndicate pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyndicateContribution {
    pub member: Address,
    /// Stake this member is contributing to the pool, in stroops.
    pub amount: i128,
}

/// A pool of vouchers who share vouching risk and reward proportionally to
/// their contributed stake, instead of each voucher bearing risk alone.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyndicatePool {
    pub pool_id: u64,
    pub creator: Address,
    pub token: Address,
    pub members: Vec<Address>,
    /// Sum of all members' `amount` contributions, in stroops.
    pub total_stake: i128,
    /// Reward accrued to the pool that has not yet been distributed, in stroops.
    pub pending_rewards: i128,
    pub created_at: u64,
    pub active: bool,
}

/// Per-member record within a syndicate pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyndicateMember {
    pub member: Address,
    /// Stake contributed by this member, in stroops.
    pub contribution: i128,
    /// This member's share of the pool in basis points (10_000 = 100%).
    pub share_bps: u32,
    /// Cumulative rewards this member has been paid out, in stroops.
    pub rewards_received: i128,
}

/// Running performance metrics for a syndicate pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyndicatePerformance {
    pub pool_id: u64,
    /// Total rewards distributed to members across the pool's lifetime, in stroops.
    pub total_rewards_distributed: i128,
    /// Total stake lost to slashing across the pool's lifetime, in stroops.
    pub total_slashed: i128,
    /// Number of times rewards have been distributed.
    pub distribution_count: u32,
}

/// Governance proposal status for syndicate member voting.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SyndicateProposalStatus {
    Pending,
    Approved,
    Rejected,
    /// #1409: the approved action has been carried out (pool dissolved,
    /// principal returned to members). Terminal — execution can never run
    /// twice against the same proposal.
    Executed,
}

/// A member-raised governance proposal within a syndicate pool (e.g. dissolve
/// the pool, change a policy). Voting weight is each member's `share_bps`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyndicateProposal {
    pub pool_id: u64,
    pub proposal_id: u64,
    pub proposer: Address,
    pub description: String,
    /// Sum of share_bps of members who voted for.
    pub votes_for_bps: u32,
    /// Sum of share_bps of members who voted against.
    pub votes_against_bps: u32,
    pub status: SyndicateProposalStatus,
    pub created_at: u64,
}

// ── Issue #1169: Milestone-based vouch release ────────────────────────────────

/// Loan lifecycle milestones that a vouch can be partially released against.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LoanMilestone {
    Issued,
    FirstPaymentMade,
    HalfRepaid,
    Completed,
}

impl LoanMilestone {
    /// Fraction of a voucher's stake released when this milestone is reached,
    /// expressed in basis points. Each milestone releases 25% (2_500 bps).
    pub fn release_bps(&self) -> u32 {
        2_500
    }

    /// Stable numeric discriminant used as a storage-key component.
    pub fn index(&self) -> u32 {
        match self {
            LoanMilestone::Issued => 0,
            LoanMilestone::FirstPaymentMade => 1,
            LoanMilestone::HalfRepaid => 2,
            LoanMilestone::Completed => 3,
        }
    }
}

// ── Issue #1168: Recurring repayment automation ───────────────────────────────

/// Borrower-configured recurring repayment schedule, executed by anyone
/// (e.g. an off-chain keeper) once `next_payment_due` has passed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecurringPaymentConfig {
    pub borrower: Address,
    pub token: Address,
    /// Amount transferred per period, in stroops.
    pub amount: i128,
    /// Seconds between successive payments.
    pub frequency_secs: u64,
    /// Ledger timestamp the schedule starts at.
    pub start_date: u64,
    /// Ledger timestamp the next payment becomes executable.
    pub next_payment_due: u64,
    pub active: bool,
    pub success_count: u32,
    pub failure_count: u32,
    pub retry_count: u32,
}

// ── Issue #1241: Governance Token with DAO Voting ─────────────────────────────

/// Governance token record for a holder.
/// 1 GOV token = 1 vote. Balances are tracked as i128 (smallest unit).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovTokenBalance {
    /// The token holder.
    pub holder: Address,
    /// Balance of GOV tokens in smallest unit.
    pub balance: i128,
    /// Timestamp of first token receipt (used for participation metrics).
    pub first_received_at: u64,
    /// Total governance votes cast by this holder.
    pub votes_cast: u32,
}

/// A DAO governance proposal that requires 1% of total GOV supply to create.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaoProposal {
    /// Unique proposal ID.
    pub id: u64,
    /// Address of the proposer.
    pub proposer: Address,
    /// Human-readable description of the proposal.
    pub description: soroban_sdk::String,
    /// Total votes FOR the proposal (in GOV tokens).
    pub votes_for: i128,
    /// Total votes AGAINST the proposal (in GOV tokens).
    pub votes_against: i128,
    /// Voters who have cast a vote: (voter → for/against).
    pub voters: Vec<Address>,
    /// Current status of the proposal.
    pub status: DaoProposalStatus,
    /// Timestamp when the proposal was created.
    pub created_at: u64,
    /// Timestamp when the voting period ends.
    pub voting_ends_at: u64,
    /// Timestamp when the proposal can be executed (after voting period + timelock).
    pub executable_at: u64,
}

/// Status of a DAO governance proposal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaoProposalStatus {
    /// Proposal is accepting votes.
    Active,
    /// Voting period ended; quorum met and majority voted FOR.
    Passed,
    /// Voting period ended; quorum not met or majority voted AGAINST.
    Failed,
    /// Proposal executed on-chain.
    Executed,
    /// Proposal cancelled by proposer or admin.
    Cancelled,
}

/// Vote delegation record: a GOV holder delegates their voting power to another address.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovDelegation {
    /// The delegating address.
    pub delegator: Address,
    /// The delegate receiving the voting power.
    pub delegate: Address,
    /// Timestamp when delegation was set.
    pub set_at: u64,
}

/// Governance participation metrics tracked on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovParticipationMetrics {
    /// Total GOV tokens minted (supply).
    pub total_supply: i128,
    /// Total number of DAO proposals created.
    pub proposals_created: u64,
    /// Total number of votes cast across all proposals.
    pub total_votes_cast: u64,
    /// Number of unique voters who have participated.
    pub unique_voters: u32,
}

/// Minimum GOV token threshold to create a proposal, in basis points of total supply
/// (100 = 1%).
pub const GOV_PROPOSAL_THRESHOLD_BPS: i128 = 100;
/// BPS denominator for GOV calculations.
pub const GOV_BPS_DENOMINATOR: i128 = 10_000;
/// Default DAO voting period in seconds (7 days).
pub const DAO_VOTING_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;
/// Default DAO timelock after voting before execution, in seconds (2 days).
pub const DAO_TIMELOCK_SECS: u64 = 2 * 24 * 60 * 60;
/// Quorum: percentage of total supply that must vote, in basis points (1000 = 10%).
pub const GOV_QUORUM_BPS: i128 = 1_000;

// ── Issue #1243: Dynamic Interest Rate Based on Utilization ───────────────────

/// Configuration for the utilization-based dynamic interest rate model.
///
/// Rate formula:
///   - When utilization ≤ `kink_utilization_bps / 10_000`:
///       rate = base_rate_bps
///   - When utilization > `kink_utilization_bps / 10_000`:
///       excess = utilization_bps - kink_utilization_bps
///       rate = base_rate_bps + (excess * premium_slope_bps / 10_000)
///   - Capped at `rate_cap_bps`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilizationRateConfig {
    /// Whether utilization-based rate is active.
    pub enabled: bool,
    /// Base interest rate when utilization is low, in basis points (e.g. 200 = 2%).
    pub base_rate_bps: i128,
    /// Utilization percentage at which the premium slope kicks in, in basis points
    /// (e.g. 8000 = 80%).
    pub kink_utilization_bps: i128,
    /// Slope of the interest rate above the kink, in basis points per basis-point of
    /// excess utilization (e.g. 300 means each 1% excess utilization adds 3 bps to rate).
    pub premium_slope_bps: i128,
    /// Maximum possible interest rate, in basis points (rate cap, e.g. 5000 = 50%).
    pub rate_cap_bps: i128,
    /// Minimum possible interest rate, in basis points (rate floor, e.g. 50 = 0.5%).
    pub rate_floor_bps: i128,
}

/// A snapshot of a utilization rate change, for tracking history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilizationRateSnapshot {
    /// Ledger timestamp of this snapshot.
    pub recorded_at: u64,
    /// Utilization at this point, in basis points (0–10000).
    pub utilization_bps: i128,
    /// Effective rate at this point, in basis points.
    pub effective_rate_bps: i128,
    /// Total outstanding loan principal at this point, in stroops.
    pub outstanding_loans: i128,
    /// Total capital (vouched stake) at this point, in stroops.
    pub total_capital: i128,
}

/// Default utilization rate configuration.
pub fn default_utilization_rate_config() -> UtilizationRateConfig {
    UtilizationRateConfig {
        enabled: true,
        base_rate_bps: 200,          // 2% base rate
        kink_utilization_bps: 8_000, // kink at 80% utilization
        premium_slope_bps: 300,      // 3 bps per 1% excess utilization above kink
        rate_cap_bps: 5_000,         // cap at 50%
        rate_floor_bps: 50,          // floor at 0.5%
    }
}

// ── Issue #1245: Loyalty Program with Tiered Rewards ──────────────────────────

/// Loyalty tier for a user based on total successful loan repayments.
///
/// Tiers:
///   - Bronze: 0–4 repayments
///   - Silver: 5–19 repayments
///   - Gold:   20+ repayments
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoyaltyTier {
    /// 0–4 successful repayments.
    Bronze,
    /// 5–19 successful repayments.
    Silver,
    /// 20+ successful repayments.
    Gold,
}

/// Benefits associated with each loyalty tier.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoyaltyBenefits {
    /// Interest rate discount in basis points (e.g. 50 = 0.5% reduction).
    pub interest_rate_discount_bps: i128,
    /// Protocol fee waiver in basis points (e.g. 10000 = 100% waiver = full fee waiver).
    pub fee_waiver_bps: u32,
    /// Minimum stake discount in basis points (e.g. 500 = 5% lower minimum stake).
    pub min_stake_discount_bps: u32,
    /// Annual anniversary bonus in basis points (e.g. 100 = 1% bonus on next repayment yield).
    pub anniversary_bonus_bps: u32,
}

/// A user's loyalty program record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoyaltyRecord {
    /// User address.
    pub user: Address,
    /// Current loyalty tier.
    pub tier: LoyaltyTier,
    /// Number of successful loan repayments.
    pub repayment_count: u32,
    /// Timestamp of account registration / first loan.
    pub member_since: u64,
    /// Timestamp of last tier upgrade.
    pub last_tier_upgrade_at: u64,
    /// Timestamp of last anniversary bonus claimed.
    pub last_anniversary_bonus_at: u64,
    /// Total loyalty benefits earned (cumulative interest saved, in stroops).
    pub total_benefits_earned: i128,
}

/// Repayment thresholds for tier advancement.
pub const LOYALTY_SILVER_THRESHOLD: u32 = 5;
pub const LOYALTY_GOLD_THRESHOLD: u32 = 20;

/// Default Bronze tier benefits.
pub fn loyalty_bronze_benefits() -> LoyaltyBenefits {
    LoyaltyBenefits {
        interest_rate_discount_bps: 0,
        fee_waiver_bps: 0,
        min_stake_discount_bps: 0,
        anniversary_bonus_bps: 0,
    }
}

/// Default Silver tier benefits.
pub fn loyalty_silver_benefits() -> LoyaltyBenefits {
    LoyaltyBenefits {
        interest_rate_discount_bps: 50,   // 0.5% interest discount
        fee_waiver_bps: 2_500,            // 25% fee waiver
        min_stake_discount_bps: 500,      // 5% lower minimum stake
        anniversary_bonus_bps: 50,        // 0.5% anniversary bonus
    }
}

/// Default Gold tier benefits.
pub fn loyalty_gold_benefits() -> LoyaltyBenefits {
    LoyaltyBenefits {
        interest_rate_discount_bps: 150,  // 1.5% interest discount
        fee_waiver_bps: 10_000,           // 100% fee waiver
        min_stake_discount_bps: 1_500,    // 15% lower minimum stake
        anniversary_bonus_bps: 150,       // 1.5% anniversary bonus
    }
}

/// Anniversary period in seconds (365 days).
pub const LOYALTY_ANNIVERSARY_PERIOD_SECS: u64 = 365 * 24 * 60 * 60;

// ── Issue #1075: Non-Stellar token bridge metadata ─────────────────────────────

/// Metadata for a token bridged in from a non-Stellar chain.
#[contracttype]
#[derive(Clone)]
pub struct TokenBridgeMetadata {
    /// The local (Stellar) address representing the bridged token.
    pub token_address: Address,
    /// The bridge contract responsible for this token.
    pub bridge_contract: Address,
    /// The token's address on its origin chain.
    pub source_token_address: Address,
    /// Chain ID of the token's origin chain.
    pub source_chain_id: u32,
    /// Conversion price in basis points relative to the primary protocol token.
    pub price_bps: i128,
    /// Timestamp of the last price update.
    pub price_updated_at: u64,
    /// Whether this bridged token is currently accepted.
    pub enabled: bool,
    /// Maximum balance this contract will hold of the bridged token (0 = unlimited).
    pub max_balance_cap: i128,
}

// ── Issue #1076: Token swap on repayment mismatch ──────────────────────────────

/// Configuration allowing a borrower to repay a loan in an alternative token via DEX swap.
#[contracttype]
#[derive(Clone)]
pub struct TokenSwapConfig {
    /// Loan this configuration applies to.
    pub loan_id: u64,
    /// The loan's primary denomination token.
    pub primary_token: Address,
    /// Tokens the borrower may repay with instead of the primary token.
    pub allowed_swap_tokens: Vec<Address>,
    /// DEX contract used to perform the swap.
    pub dex_contract: Address,
    /// Maximum acceptable slippage, in basis points.
    pub max_slippage_bps: i128,
    /// Whether swaps are currently enabled for this loan.
    pub swaps_enabled: bool,
    /// Timestamp this configuration was created.
    pub created_at: u64,
}

/// Default yield bonus (basis points) per liquidity tier (0 = highest liquidity, 3 = lowest).
pub const DEFAULT_LIQUIDITY_TIER_BONUSES: [i128; 4] = [0, 50, 100, 200];
