use soroban_sdk::{contracterror, contracttype, String, Env};

// ── Issue #109: Structured ApiError shape ────────────────────────────────────

/// A structured error payload that can be surfaced to off-chain clients.
///
/// When a contract function returns a `ContractError`, integrators can convert
/// it into an `ApiError` for consistent JSON/API response shaping:
///
/// ```json
/// {
///   "code": 1,
///   "message": "InsufficientFunds",
///   "details": "Total vouched stake is below the requested loan threshold."
/// }
/// ```
///
/// # Fields
/// - `code`    — The stable numeric error code matching `ContractError` discriminants.
/// - `message` — Short, human-readable error name (matches the enum variant name).
/// - `details` — Optional extended description, may contain context-specific hints.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ApiError {
    /// Stable numeric code — identical to the `ContractError` u32 discriminant.
    pub code: u32,
    /// Short human-readable error label (e.g. `"InsufficientFunds"`).
    pub message: String,
    /// Optional extended description with resolution hints.
    /// Empty string means no extra detail is available.
    pub details: String,
}

impl ApiError {
    /// Convert a `ContractError` into a structured `ApiError` with a stable
    /// code, a canonical message string, and an optional detail hint.
    pub fn from_contract_error(env: &Env, err: ContractError) -> Self {
        let (message, details) = contract_error_meta(env, err);
        ApiError {
            code: err as u32,
            message,
            details,
        }
    }
}

/// Returns `(message, details)` strings for each known `ContractError` variant.
/// Extracted into a free function so it can be used both in `ApiError` and in
/// off-chain tooling that imports this crate.
pub fn contract_error_meta(env: &Env, err: ContractError) -> (String, String) {
    match err {
        ContractError::InsufficientFunds => (
            String::from_str(env, "InsufficientFunds"),
            String::from_str(env, "Stake or amount is zero/negative; or contract balance is insufficient for disbursement."),
        ),
        ContractError::ActiveLoanExists => (
            String::from_str(env, "ActiveLoanExists"),
            String::from_str(env, "Borrower already has an active loan. Wait for repayment or default resolution."),
        ),
        ContractError::StakeOverflow => (
            String::from_str(env, "StakeOverflow"),
            String::from_str(env, "Summing vouched stakes would overflow i128. Reduce stake amounts."),
        ),
        ContractError::ZeroAddress => (
            String::from_str(env, "ZeroAddress"),
            String::from_str(env, "Admin or token address is the zero address. Provide a valid address."),
        ),
        ContractError::DuplicateVouch => (
            String::from_str(env, "DuplicateVouch"),
            String::from_str(env, "Voucher already has an active vouch for this borrower. Use increase_stake() instead."),
        ),
        ContractError::NoActiveLoan => (
            String::from_str(env, "NoActiveLoan"),
            String::from_str(env, "No active loan found for borrower. Verify the borrower address."),
        ),
        ContractError::ContractPaused => (
            String::from_str(env, "ContractPaused"),
            String::from_str(env, "Contract is paused. Wait for an admin to call unpause()."),
        ),
        ContractError::LoanPastDeadline => (
            String::from_str(env, "LoanPastDeadline"),
            String::from_str(env, "Repayment deadline has passed. Use slash() to mark default."),
        ),
        ContractError::PoolLengthMismatch => (
            String::from_str(env, "PoolLengthMismatch"),
            String::from_str(env, "Pool borrower/stake arrays have different lengths."),
        ),
        ContractError::PoolEmpty => (
            String::from_str(env, "PoolEmpty"),
            String::from_str(env, "Pool has no members."),
        ),
        ContractError::PoolBorrowerActiveLoan => (
            String::from_str(env, "PoolBorrowerActiveLoan"),
            String::from_str(env, "A pool member already has an active loan."),
        ),
        ContractError::PoolInsufficientFunds => (
            String::from_str(env, "PoolInsufficientFunds"),
            String::from_str(env, "Pool has insufficient funds for the requested operation."),
        ),
        ContractError::MinStakeNotMet => (
            String::from_str(env, "MinStakeNotMet"),
            String::from_str(env, "Vouch stake is below the admin-configured minimum. Increase stake."),
        ),
        ContractError::LoanExceedsMaxAmount => (
            String::from_str(env, "LoanExceedsMaxAmount"),
            String::from_str(env, "Requested loan exceeds admin-configured maximum. Request a smaller amount."),
        ),
        ContractError::InsufficientVouchers => (
            String::from_str(env, "InsufficientVouchers"),
            String::from_str(env, "Number of vouchers is below admin-configured minimum. Recruit more vouchers."),
        ),
        ContractError::UnauthorizedCaller => (
            String::from_str(env, "UnauthorizedCaller"),
            String::from_str(env, "Caller is not authorized for this operation. Ensure the correct address signs the transaction."),
        ),
        ContractError::InvalidAmount => (
            String::from_str(env, "InvalidAmount"),
            String::from_str(env, "Numeric parameter fails validity check. Pass a value in the documented valid range."),
        ),
        ContractError::InvalidStateTransition => (
            String::from_str(env, "InvalidStateTransition"),
            String::from_str(env, "Operation is not valid for the current loan status. Check loan_status() first."),
        ),
        ContractError::AlreadyInitialized => (
            String::from_str(env, "AlreadyInitialized"),
            String::from_str(env, "Contract is already initialized. initialize() is one-time only."),
        ),
        ContractError::VouchTooRecent => (
            String::from_str(env, "VouchTooRecent"),
            String::from_str(env, "Vouch was added too recently. Wait for the vouch age requirement to pass."),
        ),
        ContractError::VouchCooldownActive => (
            String::from_str(env, "VouchCooldownActive"),
            String::from_str(env, "Vouch cooldown is still active. Wait before vouching again."),
        ),
        ContractError::VoucherNotWhitelisted => (
            String::from_str(env, "VoucherNotWhitelisted"),
            String::from_str(env, "Voucher address is not in the whitelist. Contact the admin."),
        ),
        ContractError::Blacklisted => (
            String::from_str(env, "Blacklisted"),
            String::from_str(env, "Address is blacklisted. Contact the protocol admin."),
        ),
        ContractError::TimelockNotFound => (
            String::from_str(env, "TimelockNotFound"),
            String::from_str(env, "Timelock operation ID not found. Verify the ID returned when operation was queued."),
        ),
        ContractError::TimelockNotReady => (
            String::from_str(env, "TimelockNotReady"),
            String::from_str(env, "Timelock delay has not elapsed. Wait and retry."),
        ),
        ContractError::TimelockExpired => (
            String::from_str(env, "TimelockExpired"),
            String::from_str(env, "Timelock operation has expired. Re-queue the operation."),
        ),
        ContractError::NoVouchesForBorrower => (
            String::from_str(env, "NoVouchesForBorrower"),
            String::from_str(env, "No vouches found for this borrower. Verify the borrower address."),
        ),
        ContractError::VoucherNotFound => (
            String::from_str(env, "VoucherNotFound"),
            String::from_str(env, "Voucher address not found in borrower's vouch list."),
        ),
        ContractError::InvalidToken => (
            String::from_str(env, "InvalidToken"),
            String::from_str(env, "Token is not allowed or does not implement SEP-41. Use get_config() to list allowed tokens."),
        ),
        ContractError::AlreadyVoted => (
            String::from_str(env, "AlreadyVoted"),
            String::from_str(env, "Voucher has already voted on this slash proposal. Each voucher votes once."),
        ),
        ContractError::SlashVoteNotFound => (
            String::from_str(env, "SlashVoteNotFound"),
            String::from_str(env, "No open slash proposal for this borrower. Initiate a slash vote first."),
        ),
        ContractError::SlashAlreadyExecuted => (
            String::from_str(env, "SlashAlreadyExecuted"),
            String::from_str(env, "Slash already executed for this borrower. No further action needed."),
        ),
        ContractError::LoanBelowMinAmount => (
            String::from_str(env, "LoanBelowMinAmount"),
            String::from_str(env, "Requested loan is below the admin-configured minimum. Request a larger amount."),
        ),
        ContractError::QuorumNotMet => (
            String::from_str(env, "QuorumNotMet"),
            String::from_str(env, "Slash vote quorum not reached. Recruit more voucher votes."),
        ),
        ContractError::DelayNotElapsed => (
            String::from_str(env, "DelayNotElapsed"),
            String::from_str(env, "Required delay has not elapsed. Wait and retry."),
        ),
        ContractError::MaxVouchersPerBorrowerExceeded => (
            String::from_str(env, "MaxVouchersPerBorrowerExceeded"),
            String::from_str(env, "Borrower has reached the maximum number of vouchers allowed."),
        ),
        ContractError::InsufficientVoucherBalance => (
            String::from_str(env, "InsufficientVoucherBalance"),
            String::from_str(env, "Voucher has insufficient token balance to cover the requested stake."),
        ),
        ContractError::SelfVouchNotAllowed => (
            String::from_str(env, "SelfVouchNotAllowed"),
            String::from_str(env, "Voucher and borrower cannot be the same address."),
        ),
        ContractError::DuplicateToken => (
            String::from_str(env, "DuplicateToken"),
            String::from_str(env, "Token is already in the allowed tokens list."),
        ),
        ContractError::InvalidAdminThreshold => (
            String::from_str(env, "InvalidAdminThreshold"),
            String::from_str(env, "Admin threshold is 0 or exceeds the number of admins. Set between 1 and len(admins)."),
        ),
        ContractError::InsufficientYieldReserve => (
            String::from_str(env, "InsufficientYieldReserve"),
            String::from_str(env, "Yield reserve is insufficient to cover promised yield. Admin must pre-fund the reserve."),
        ),
        ContractError::ReminderAlreadySent => (
            String::from_str(env, "ReminderAlreadySent"),
            String::from_str(env, "Repayment reminder has already been sent for this loan."),
        ),
        ContractError::InsurancePoolEmpty => (
            String::from_str(env, "InsurancePoolEmpty"),
            String::from_str(env, "Insurance pool has no funds to cover the claim."),
        ),
        ContractError::InsuranceClaimAlreadyMade => (
            String::from_str(env, "InsuranceClaimAlreadyMade"),
            String::from_str(env, "An insurance claim has already been made for this loan."),
        ),
        ContractError::InvalidBps => (
            String::from_str(env, "InvalidBps"),
            String::from_str(env, "Basis points value is invalid. Must be in range 0–10000."),
        ),
        ContractError::WithdrawalAlreadyQueued => (
            String::from_str(env, "WithdrawalAlreadyQueued"),
            String::from_str(env, "A withdrawal is already queued for this voucher/borrower pair."),
        ),
        ContractError::WithdrawalNotQueued => (
            String::from_str(env, "WithdrawalNotQueued"),
            String::from_str(env, "No queued withdrawal found for this voucher/borrower pair."),
        ),
        ContractError::PartialWithdrawalExceedsCap => (
            String::from_str(env, "PartialWithdrawalExceedsCap"),
            String::from_str(env, "Partial withdrawal amount exceeds the 50% cap."),
        ),
        ContractError::SlashCooldownActive => (
            String::from_str(env, "SlashCooldownActive"),
            String::from_str(env, "Borrower was slashed too recently. Slash cooldown is still active."),
        ),
        ContractError::NotGovernanceParticipant => (
            String::from_str(env, "NotGovernanceParticipant"),
            String::from_str(env, "Caller is not an admin or protocol-token holder eligible to govern."),
        ),
        ContractError::VotingPeriodEnded => (
            String::from_str(env, "VotingPeriodEnded"),
            String::from_str(env, "Governance action is not allowed after the voting period has ended."),
        ),
        ContractError::ProposalNotFound => (
            String::from_str(env, "ProposalNotFound"),
            String::from_str(env, "Governance proposal not found. Verify the proposal ID."),
        ),
        ContractError::ProposalAlreadyFinalized => (
            String::from_str(env, "ProposalAlreadyFinalized"),
            String::from_str(env, "Governance proposal has already been finalized."),
        ),
        ContractError::InvalidThreshold => (
            String::from_str(env, "InvalidThreshold"),
            String::from_str(env, "Invalid credential multi-signature threshold. Must be > 0 and <= number of signers."),
        ),
        ContractError::InsufficientSignatures => (
            String::from_str(env, "InsufficientSignatures"),
            String::from_str(env, "Insufficient signatures for multi-signature verification."),
        ),
        ContractError::UnauthorizedSigner => (
            String::from_str(env, "UnauthorizedSigner"),
            String::from_str(env, "Signer is not authorized to sign this credential."),
        ),
        ContractError::SignatureError => (
            String::from_str(env, "SignatureError"),
            String::from_str(env, "Signature verification failed."),
        ),
        ContractError::CredentialRateLimitExceeded => (
            String::from_str(env, "CredentialRateLimitExceeded"),
            String::from_str(env, "Credential verification rate limit exceeded for this holder."),
        ),
        ContractError::InvalidRateLimit => (
            String::from_str(env, "InvalidRateLimit"),
            String::from_str(env, "Invalid rate limit configuration. Limit must be positive."),
        ),
        ContractError::MetadataTamperDetected => (
            String::from_str(env, "MetadataTamperDetected"),
            String::from_str(env, "Credential metadata tampering detected. Metadata hash mismatch."),
        ),
        ContractError::MetadataHashMismatch => (
            String::from_str(env, "MetadataHashMismatch"),
            String::from_str(env, "Credential metadata hash does not match. Metadata may have been modified."),
        ),
        ContractError::QuorumSliceNotFound => (
            String::from_str(env, "QuorumSliceNotFound"),
            String::from_str(env, "Quorum slice not found."),
        ),
        ContractError::SelfHealingDisabled => (
            String::from_str(env, "SelfHealingDisabled"),
            String::from_str(env, "Self-healing is not enabled for this slice."),
        ),
        ContractError::InsufficientQuorum => (
            String::from_str(env, "InsufficientQuorum"),
            String::from_str(env, "Insufficient quorum to perform self-healing."),
        ),
        ContractError::SbtGroupNotFound => (
            String::from_str(env, "SbtGroupNotFound"),
            String::from_str(env, "No group SBT record found for the requested group_id."),
        ),
        ContractError::SbtGroupProposalNotFound => (
            String::from_str(env, "SbtGroupProposalNotFound"),
            String::from_str(env, "No membership proposal found for the requested (group_id, proposal_id)."),
        ),
        ContractError::SbtLineageNotFound => (
            String::from_str(env, "SbtLineageNotFound"),
            String::from_str(env, "No lineage node found for the requested sbt_id."),
        ),
        _ => (
            String::from_str(env, "UnknownError"),
            String::from_str(env, "An unknown error occurred."),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    InsufficientFunds = 1,
    ActiveLoanExists = 2,
    StakeOverflow = 3,
    ZeroAddress = 4,
    DuplicateVouch = 5,
    NoActiveLoan = 6,
    ContractPaused = 7,
    LoanPastDeadline = 8,
    PoolLengthMismatch = 9,
    PoolEmpty = 10,
    PoolBorrowerActiveLoan = 11,
    PoolInsufficientFunds = 12,
    MinStakeNotMet = 13,
    LoanExceedsMaxAmount = 14,
    InsufficientVouchers = 15,
    UnauthorizedCaller = 16,
    InvalidAmount = 17,
    InvalidStateTransition = 18,
    AlreadyInitialized = 19,
    VouchTooRecent = 20,
    VouchCooldownActive = 21,
    VoucherNotWhitelisted = 23,
    Blacklisted = 24,
    TimelockNotFound = 25,
    TimelockNotReady = 26,
    TimelockExpired = 27,
    NoVouchesForBorrower = 28,
    VoucherNotFound = 29,
    InvalidToken = 30,
    AlreadyVoted = 31,
    SlashVoteNotFound = 32,
    SlashAlreadyExecuted = 33,
    LoanBelowMinAmount = 34,
    QuorumNotMet = 35,
    DelayNotElapsed = 36,
    MaxVouchersPerBorrowerExceeded = 37,
    InsufficientVoucherBalance = 38,
    SelfVouchNotAllowed = 39,
    DuplicateToken = 40,
    InvalidAdminThreshold = 41,
    InsufficientYieldReserve = 42,
    ReminderAlreadySent = 43,
    /// Insurance pool has no funds to cover the claim.
    InsurancePoolEmpty = 44,
    /// Admin contribution exceeds `insurance_fund_max_contribution` cap (#1437).
    InsuranceContributionTooLarge = 221,
    /// Insurance claim already made for this loan.
    InsuranceClaimAlreadyMade = 45,
    /// Basis points value is invalid (must be 0–10000).
    InvalidBps = 46,
    /// Withdrawal request already queued for this voucher/borrower pair.
    WithdrawalAlreadyQueued = 57,
    /// No queued withdrawal found for this voucher/borrower pair.
    WithdrawalNotQueued = 47,
    /// Partial withdrawal amount exceeds the 50% cap.
    PartialWithdrawalExceedsCap = 48,
    /// Borrower was slashed too recently; slash cooldown is still active.
    SlashCooldownActive = 49,
    /// Caller is not an admin or protocol-token holder allowed to govern.
    NotGovernanceParticipant = 50,
    /// Governance action is not allowed after the voting period has ended.
    VotingPeriodEnded = 51,
    /// Governance proposal was not found.
    ProposalNotFound = 52,
    /// Governance proposal was already finalized.
    ProposalAlreadyFinalized = 53,
    /// Oracle caller is not the registered oracle contract (#666/#667).
    OracleUnauthorized = 54,
    /// Repayment retry limit has been exceeded (#669).
    MaxRetriesExceeded = 55,
    /// No escrow record found for this borrower (#666/#667).
    NoEscrowFound = 56,
    /// No slash record found for the given slash ID.
    SlashRecordNotFound = 149,
    /// Refinancing was attempted without any outstanding balance to settle.
    RefinanceNoOutstanding = 150,
    /// Slash has already been reversed and cannot be reversed again.
    SlashAlreadyReversed = 58,
    /// Caller has exceeded the configured rate limit.
    RateLimitExceeded = 59,
    /// Caller does not have the required role or permission.
    PermissionDenied = 60,
    /// Cryptographic proof validation failed.
    InvalidProof = 61,
    /// Arithmetic overflow or underflow occurred.
    ArithmeticError = 62,
    /// No rollback snapshot found for the requested deployment index (#744).
    RollbackSnapshotNotFound = 63,
    /// Admin address is not on the whitelist.
    AdminNotWhitelisted = 64,
    /// Admin address is on the blacklist.
    AdminBlacklisted = 65,
    /// Reentrancy detected — a guarded function was re-entered before the lock was released.
    Reentrancy = 66,
    /// Borrower is immune from being slashed (e.g. repaid within grace period).
    BorrowerImmune = 67,
    /// Target admin has already been revoked and cannot be revoked again.
    AdminAlreadyRevoked = 68,
    /// The target of revocation is not a current admin.
    AdminNotFound = 69,
    /// The chain_id used in a cross-chain vouch is not registered or is inactive.
    InvalidChain = 98,
    /// A bridge for this chain_id has already been registered.
    BridgeAlreadyRegistered = 99,
    /// No Ed25519 verification key is configured for the origin chain.
    BridgeNotConfigured = 100,
    /// The origin/destination chain combination is invalid.
    InvalidBridgeChain = 101,
    /// This origin-chain nonce has already been consumed.
    ReplayAttackDetected = 102,
    /// The attestation is outside the accepted freshness window.
    AttestationExpired = 103,
    /// The attestation timestamp is too far ahead of the ledger clock.
    AttestationFromFuture = 104,
    /// This canonical loan has already moved its reputation to another chain.
    ReputationAlreadySpent = 105,
    /// A newer reputation attestation has already been applied.
    StaleBridgeAttestation = 106,
    /// Governance proposal has already been approved.
    ProposalAlreadyApproved = 107,
    /// Governance proposal has expired.
    ProposalExpired = 108,
    /// Governance proposal timelock delay has not elapsed.
    TimelockDelayNotElapsed = 109,
    /// Governance proposal execution window has passed.
    ExecutionWindowPassed = 110,
    /// Governance action is invalid or not supported.
    InvalidGovernanceAction = 111,
    /// Credit score calculation failed.
    CreditScoreCalculationFailed = 112,
    /// Invalid credit score tier.
    InvalidCreditTier = 113,
    /// Credit score not found for borrower.
    CreditScoreNotFound = 114,
    /// Credit score configuration is invalid.
    InvalidCreditConfig = 115,
/// A write operation was attempted while the contract is in the Thawing state.
/// Only reads and withdrawals are permitted during a thaw period.
ContractThawing = 116,

/// Syndication not found.
SyndicationNotFound = 117,
/// Syndication member not found.
SyndicationMemberNotFound = 118,
/// Syndication already has a loan.
SyndicationHasLoan = 119,
/// Syndication is not in the correct status.
InvalidSyndicationStatus = 120,
/// Syndication member already exists.
SyndicationMemberExists = 121,
/// Syndication has insufficient approvals.
InsufficientSyndicationApprovals = 122,
/// Syndication has too many members.
SyndicationMaxMembersExceeded = 123,
/// Syndication has too few members.
SyndicationMinMembersNotMet = 124,
/// Invalid syndication share percentage.
InvalidSyndicationShare = 125,
/// Syndication configuration is invalid.
InvalidSyndicationConfig = 126,
/// No slash escrow found for this borrower.
AppealNotFound = 127,
/// Voucher has already voted on this appeal.
AppealAlreadyVoted = 128,
/// Appeal quorum (2/3 voucher stake) not met to overturn slash.
AppealQuorumNotMet = 129,
/// Escrow period has expired; appeal can no longer be filed or voted on.
EscrowExpired = 130,
/// Emergency cooldown bypass is not authorised for this voucher.
EmergencyBypassNotAuthorised = 131,
    /// Cooldown bypass request already exists for this (borrower, voucher) pair.
    CooldownBypassAlreadyRequested = 174,
    /// Cooldown bypass request not found.
    CooldownBypassNotFound = 175,
    /// Cooldown bypass has already been approved.
    CooldownBypassAlreadyApproved = 176,
    /// Insufficient admin approvals for cooldown bypass (need 2/3).
    CooldownBypassInsufficientApprovals = 177,
    /// Cross-collateral pool not found.
    CollateralPoolNotFound = 132,
    /// Cross-collateral pool is already active (has an assigned borrower).
    CollateralPoolActive = 133,
    /// Caller is not a member of the specified collateral pool.
    NotPoolMember = 134,
    /// Gradual-unstake schedule not found for this voucher/borrower pair.
    GradualUnstakeNotFound = 135,
    /// A gradual-unstake schedule is already active for this pair.
    GradualUnstakeAlreadyActive = 136,
    /// The next instalment is not yet due.
    GradualUnstakeNotDue = 137,
    /// Loan extension request already pending for this borrower.
    ExtensionAlreadyRequested = 138,
    /// Maximum number of extensions per loan has been reached.
    MaxExtensionsReached = 139,
    /// Caller does not have permission to view this loan (privacy restriction).
    LoanPrivacyRestricted = 140,
    /// Insurance pool is not connected to this loan.
    InsuranceNotLinked = 141,
    /// No relay verification key is configured for the source chain.
    RelayKeyNotConfigured = 154,
    /// Relay chain id is zero or otherwise invalid.
    InvalidRelayChain = 155,
    /// A relay attestation reused an already-consumed nonce.
    RelayReplayDetected = 156,
    /// The relay attestation is older than the freshness window allows.
    RelayEventExpired = 157,
    /// The relay attestation is timestamped too far in the future.
    RelayEventFromFuture = 158,
    /// A relay event with this (source chain, sequence) was already processed.
    RelayEventAlreadyProcessed = 159,
    /// A relay acknowledgement tried to move the cursor backwards.
    RelayAckRegression = 160,
    /// A relay attestation's signature did not verify against the registered key.
    InvalidRelaySignature = 161,
    /// Circular delegation chain detected in vote delegation.
    CircularDelegation = 162,
    /// Delegation not found.
    DelegationNotFound = 163,
    /// Loan has already been fully repaid.
    AlreadyRepaid = 164,
    /// Loan amount exceeds the maximum ratio allowed.
    LoanExceedsMaxRatio = 165,
    /// Self-co-borrowing is not allowed.
    SelfCoBorrowerNotAllowed = 166,
    /// Maximum number of co-borrowers exceeded.
    MaxCoBorrowersExceeded = 167,
    /// Co-borrower is already added to this loan.
    CoBorrowerAlreadyAdded = 168,
    /// Operation is not allowed on a loan in forbearance.
    LoanInForbearance = 169,
    /// No forbearance record found for this loan.
    ForbearanceNotFound = 170,
    /// Forbearance is not currently active.
    ForbearanceNotActive = 171,
    /// Maximum number of forbearance periods reached.
    MaxForbearanceExceeded = 172,
    /// Invalid configuration for dynamic interest rate.
    InvalidDynamicRateConfig = 173,
    /// Attestor reported fewer origin-chain confirmations than the required minimum.
    InsufficientBridgeConfirmations = 220,
    /// A live protocol invariant check failed (see `crate::invariants`).
    InvariantViolation = 178,
    /// The withdrawal queue has reached its maximum size.
    WithdrawalQueueFull = 179,
    /// A vouch split would leave the parent or child vouch below the
    /// minimum split amount.
    SplitBelowMinimum = 180,
    /// A vouch rotation was attempted before the cooling-off period elapsed.
    RotationCooldownActive = 181,
    /// A large-loan approval proposal was not found for the given id.
    LargeLoanApprovalNotFound = 182,
    /// A large-loan approval proposal has passed its 48-hour expiration window.
    LargeLoanApprovalExpired = 183,
    /// A large-loan approval proposal has already collected enough signatures
    /// and been executed; it cannot be signed or executed again.
    LargeLoanApprovalAlreadyExecuted = 184,
    /// The same admin attempted to sign a large-loan approval proposal twice.
    DuplicateApprovalSigner = 185,
    /// A large-loan approval was proposed for an amount at or below the
    /// configured large-loan threshold, so it does not require multi-sig.
    BelowLargeLoanThreshold = 186,
    /// A slash appeal is already in progress for this borrower via the other
    /// appeal mechanism. Only one appeal mechanism (evidence-based #552 or
    /// escrow-quorum #841) may be active at a time to prevent double-refunds.
    AppealAlreadyPending = 187,
    // ── Issue #1238: Staking Pool ─────────────────────────────────────────────
    /// No staking pool found for the given pool_id.
    StakingPoolNotFound = 187,
    /// Operation requires an Active staking pool, but the pool is Draining or Closed.
    StakingPoolNotActive = 188,
    // ── Issue #1247: Referral Rewards ─────────────────────────────────────────
    /// Referral code not found or does not correspond to any registered referrer.
    ReferralCodeNotFound = 189,
    /// Caller cannot refer themselves.
    SelfReferralNotAllowed = 190,
    /// This borrower already has a referrer registered.
    ReferralAlreadyRegistered = 191,
    // ── Guarantor system ────────────────────────────────────────────────────
    /// No guarantor record found for the given loan.
    GuarantorNotFound = 192,
    /// A guarantor has already been assigned to this loan.
    GuarantorAlreadyAssigned = 193,
    /// The guarantor's obligation for this loan has already been claimed.
    GuarantorAlreadyClaimed = 194,
    /// The provided guarantor address is invalid (e.g. zero address or the borrower itself).
    InvalidGuarantor = 195,
    /// The guarantee amount is invalid (e.g. zero or exceeds the loan amount).
    InvalidGuaranteeAmount = 196,
    /// The guarantee is not in a status that permits this operation.
    InvalidGuaranteeStatus = 197,
    /// An arithmetic overflow or underflow occurred during a checked operation.
    ArithmeticOverflow = 198,
    // ── Flash loans ──────────────────────────────────────────────────────────
    /// The flash loan was not repaid (plus fee) within the same transaction.
    FlashLoanNotRepaid = 199,
    /// The requested fee amount is invalid.
    InvalidFeeAmount = 200,
    /// The requested flash loan would exceed the per-contract borrow cap.
    FlashLoanCapExceeded = 201,
    /// The requested record or resource was not found.
    NotFound = 202,
    // ── Vouch syndication ────────────────────────────────────────────────────
    /// A syndicate pool already exists for this loan.
    SyndicatePoolExists = 203,
    /// A syndicate pool cannot be created or operated on with zero members.
    SyndicateEmpty = 204,
    /// No syndicate pool was found for the given pool_id.
    SyndicatePoolNotFound = 205,
    /// The syndicate pool is not in the Active status required for this operation.
    SyndicateNotActive = 206,
    /// Caller is not a member of the specified syndicate pool.
    NotSyndicateMember = 207,
    /// Caller has already voted on this syndicate proposal.
    SyndicateAlreadyVoted = 208,
    /// No syndicate proposal was found for the given proposal_id.
    SyndicateProposalNotFound = 209,
    // ── Vouch milestones ─────────────────────────────────────────────────────
    /// The milestone condition has not yet been reached.
    MilestoneNotReached = 210,
    /// This milestone's release has already been claimed.
    MilestoneAlreadyReleased = 211,
    // ── Recurring payments ───────────────────────────────────────────────────
    /// A recurring payment schedule already exists for this borrower.
    RecurringPaymentExists = 212,
    /// No recurring payment schedule was found for this borrower.
    RecurringPaymentNotFound = 213,
    /// The recurring payment schedule is not active.
    RecurringPaymentInactive = 214,
    /// The next recurring payment is not yet due.
    RecurringPaymentNotDue = 215,
    /// The lazy slash queue has reached its maximum capacity.
    QueueFull = 216,
    // ── Cross-chain vote attestations ────────────────────────────────────────
    /// Ed25519 signature verification failed for a cross-chain vote attestation.
    InvalidVoteAttestationSignature = 217,
    /// This origin-chain vote attestation nonce has already been consumed.
    VoteAttestationNonceReused = 218,
    /// The vote attestation is outside the accepted freshness window.
    VoteAttestationExpired = 219,
    // ── Issue #10: Refinance chain limits ────────────────────────────────────
    /// The borrower has reached the maximum number of refinances allowed in a
    /// single loan chain (`max_refinances_per_loan_chain`).
    RefinanceLimitExceeded = 221,
    /// A refinance was attempted before the minimum cooldown period between
    /// consecutive refinances has elapsed.
    RefinanceCooldownActive = 222,
    // ── Issue #11: Refinance eligibility enforcement ─────────────────────────
    /// `refinance_loan` was called when the shared eligibility predicate
    /// (same logic used by `refinance_quote`) determined the borrower is not
    /// eligible for a beneficial refinance at this time.
    RefinanceNotEligible = 223,
    /// Issue #1599: Invalid credential multi-signature threshold.
    InvalidThreshold = 224,
    /// Issue #1599: Insufficient signatures for multi-signature verification.
    InsufficientSignatures = 225,
    /// Issue #1599: Signer is not authorized to sign this credential.
    UnauthorizedSigner = 226,
    /// Issue #1599: Signature verification failed.
    SignatureError = 227,
    /// Issue #1603: Credential verification rate limit exceeded.
    CredentialRateLimitExceeded = 228,
    /// Issue #1603: Invalid rate limit configuration.
    InvalidRateLimit = 229,
    /// Issue #1605: Credential metadata tampering detected.
    MetadataTamperDetected = 230,
    /// Issue #1605: Credential metadata hash mismatch.
    MetadataHashMismatch = 231,
    /// Issue #1606: Quorum slice not found.
    QuorumSliceNotFound = 232,
    /// Issue #1606: Self-healing is not enabled for this slice.
    SelfHealingDisabled = 233,
    /// Issue #1606: Insufficient quorum to perform self-healing.
    InsufficientQuorum = 234,

    // ── Issue #1739: SBT Group Ownership ─────────────────────────────────────
    /// No group SBT record found for the requested group_id.
    SbtGroupNotFound = 235,
    /// No membership proposal found for the requested (group_id, proposal_id).
    SbtGroupProposalNotFound = 236,

    // ── Issue #1741: SBT Lineage Tracking ────────────────────────────────────
    /// No lineage node found for the requested sbt_id.
    SbtLineageNotFound = 237,

    // ── Issue #1733: SBT Attribute Evolution ─────────────────────────────────
    /// No attribute record found for the requested (sbt_id, attribute) key.
    SbtAttributeNotFound = 238,
    /// The requested attribute version does not exist in the history log.
    SbtAttributeVersionNotFound = 239,

    // ── Issue #1734: SBT Privacy Modes ───────────────────────────────────────
    /// No privacy record found for the requested sbt_id.
    SbtPrivacyNotFound = 240,

    // ── Issue #1735: SBT Insurance Coverage ──────────────────────────────────
    /// No insurance record found for the requested sbt_id.
    SbtInsuranceNotFound = 241,

    // ── Issue #1736: SBT Marketplace Integration ──────────────────────────────
    /// No marketplace listing found for the requested sbt_id.
    SbtMarketplaceListingNotFound = 242,
}
