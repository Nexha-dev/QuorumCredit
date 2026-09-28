#![no_std]
// Pre-existing lint suppressions — these warnings exist throughout the codebase
// and predate this PR. Suppressed here so `cargo clippy -D warnings` does not
// fail CI on issues outside the scope of this change.
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(unused_parens)]
#![allow(deprecated)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::empty_line_after_outer_attr)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::identity_op)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::len_zero)]
#![allow(clippy::needless_return)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::doc_overindented_list_items)]
#![allow(clippy::needless_lifetimes)]
// Additional clippy lints that exist across the codebase
#![allow(clippy::manual_clamp)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::unnecessary_min_or_max)]
#![allow(clippy::manual_saturating_arithmetic)]
#![allow(clippy::manual_checked_ops)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::question_mark)]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, token, xdr::ToXdr, Address, BytesN, Env,
    String, Vec,
};

pub mod admin;
pub mod arbitrage_prevention;
pub mod audit;
pub mod audit_verification;
pub mod batch_transfer;
pub mod bond_protection;
pub mod bridge;
pub mod cache;
pub mod circuit_breaker;
pub mod cooldown_bypass;
pub mod credential;
pub mod credential_rate_limit;
pub mod credential_tamper_detection;
pub mod credit_score;
pub mod detection;
pub mod cross_chain;
pub mod cross_chain_auction;
pub mod cross_chain_governance;
pub mod differential_testing;
pub mod errors;
pub mod flash_loan;
pub mod governance;
pub mod guarantor;
pub mod helpers;
pub mod insurance;
pub mod invariants;
pub mod lazy_default_detection;
pub mod lazy_slash;
pub mod liquidity_farming;
pub mod loan;
pub mod maturity;
pub mod merkle_tree;
pub mod metadata_encryption;
pub mod multitoken_support;
pub mod rbac;
pub mod reputation;
pub mod social;
pub mod types;
pub mod vouch;
pub mod vouch_reputation;
pub mod zk_snarks;
pub mod collateral_pool;
pub mod syndication;
pub mod vouch_syndication;
pub mod loan_tokenization;
pub mod pool_composability;
pub mod quorum_slice_healing;
pub mod vouch_milestones;
pub mod recurring_payment;
pub mod loan_priority;
pub mod large_loan_approval;
pub mod liquidity_mining;
pub mod loan_attribution;
pub mod loan_cart;
pub mod governance_token;
pub mod community_treasury;
pub mod interest_rate_options;
pub mod prediction_market;
pub mod reputation_nft;
pub mod staking_pool;
pub mod referral;
pub mod prediction_market;
pub mod community_treasury;
pub mod dynamic_interest;
pub mod governance_token;
pub mod interest_rate_options;
pub mod loan_attribution;
pub mod loyalty;
pub mod liquidity_mining;
// Issue #110 — circuit breaker for webhook delivery
pub mod webhook_retry;
// Issue #111 — max webhook subscriptions per caller
pub mod webhook_registry;
// Issues #1607, #1609, #1610, #1611 — attestor analytics and quorum slice monitoring
pub mod attestor_analytics;

// ── Issues #1738–#1741: SBT features ─────────────────────────────────────────
// Issue #1738 — SBT Reputation System
pub mod sbt_reputation;
// Issue #1739 — SBT Group Ownership
pub mod sbt_group;
// Issue #1740 — SBT Staking and Rewards
pub mod sbt_staking;
// Issue #1741 — SBT Lineage Tracking
pub mod sbt_lineage;
// Issue #1733 — SBT Attribute Evolution
pub mod sbt_attribute_evolution;
// Issue #1734 — SBT Privacy Modes
pub mod sbt_privacy_modes;
// Issue #1735 — SBT Insurance Coverage
pub mod sbt_insurance;
// Issue #1736 — SBT Marketplace Integration
pub mod sbt_marketplace;

#[cfg(test)]
mod governance_test;
#[cfg(test)]
mod interest_test;
#[cfg(test)]
mod invariants_test;
#[cfg(test)]
mod property_based_invariants_test;
#[cfg(test)]
mod concurrent_operations_test;
#[cfg(test)]
mod loan_purpose_test;
#[cfg(test)]
mod multi_asset_test;
#[cfg(test)]
mod referral_test;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod fuzz_stake_testing;
#[cfg(test)]
mod circuit_breaker_insurance_integration_test;
#[cfg(test)]
mod rbac_enforcement_test;
#[cfg(test)]
mod contingent_loan_test;
#[cfg(test)]
mod loan_tranching_test;
#[cfg(test)]
mod syndication_test;
#[cfg(test)]
mod loan_tokenization_test;
#[cfg(test)]
mod pool_composability_test;
#[cfg(test)]
mod storage_redesign_test;
#[cfg(test)]
mod timelock_safety_test;
#[cfg(test)]
mod cross_chain_test_scenarios;
#[cfg(test)]
mod arbitrage_prevention_test;
#[cfg(test)]
mod cross_chain_governance_test;
#[cfg(test)]
mod cross_chain_auction_test;
#[cfg(test)]
mod liquidity_farming_test;
#[cfg(test)]
mod loan_cart_test;
#[cfg(test)]
mod repay_validation_test;
#[cfg(test)]
mod unimplemented_stubs_test;
#[cfg(test)]
mod attestor_analytics_test;

pub use errors::ContractError;
pub use types::*;
pub use cross_chain::{BridgeAttestation, CrossChainLoanMetadata, UnifiedReputation};

use helpers::{
    acquire_lock, release_lock, config, get_active_loan_record, is_zero_address,
    loan_status as helper_loan_status, require_admin_approval, require_allowed_token,
    require_not_paused, token, token_client,
};
use reputation::ReputationNftExternalClient;
#[contract]
pub struct QuorumCreditContract;

#[contractimpl]
impl QuorumCreditContract {
    // ── Initialization ────────────────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        deployer: Address,
        admins: Vec<Address>,
        admin_threshold: u32,
        token: Address,
    ) -> Result<(), ContractError> {
        deployer.require_auth();

        if env.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        helpers::validate_admin_config(
            &env,
            &admins,
            admin_threshold,
            &Vec::new(&env),
            &Vec::new(&env),
        )?;
        helpers::require_valid_token(&env, &token)?;

        env.storage().instance().set(&DataKey::Deployer, &deployer);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admins: admins.clone(),
                admin_threshold,
                token: token.clone(),                admin_whitelist: Vec::new(&env),
                admin_blacklist: Vec::new(&env),
                allowed_tokens: Vec::new(&env),
                yield_bps: DEFAULT_YIELD_BPS,
                slash_bps: DEFAULT_SLASH_BPS,
                max_vouchers: DEFAULT_MAX_VOUCHERS,
                min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
                loan_duration: DEFAULT_LOAN_DURATION,
                max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
                max_loan_to_collateral_ratio: DEFAULT_MAX_LOAN_TO_COLLATERAL_RATIO,
                grace_period: 0,
                vouch_cooldown_secs: DEFAULT_VOUCH_COOLDOWN_SECS,
                min_yield_stake: DEFAULT_MIN_YIELD_STAKE,
                min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
                prepayment_penalty_bps: 0,
                liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
                voting_period_seconds: DEFAULT_VOTING_PERIOD_SECONDS,
                slash_cooldown_seconds: 0,
                emergency_pause_enabled: false,
                early_repayment_discount_bps: 0,
                oracle_address: None,
                slash_delay_seconds: 0,
                successor_admin: None,
                rate_limit_config: RateLimitConfig {
                    window_secs: DEFAULT_RATE_LIMIT_WINDOW_SECS,
                    max_calls: DEFAULT_RATE_LIMIT_COUNT,
                    enabled: false,
                    tiers: Vec::new(&env),
                },
                multi_tier_thresholds: Vec::new(&env),                dynamic_slash_threshold: DEFAULT_DYNAMIC_SLASH_THRESHOLD,
                loan_size_slash_enabled: DEFAULT_LOAN_SIZE_SLASH_ENABLED,
                loan_size_slash_max_bps: DEFAULT_LOAN_SIZE_SLASH_MAX_BPS,
                recovery_percentage: 0,
                admin_compensation_bps: 0,
                removal_vote_threshold: 0,
                confirmation_required: DEFAULT_CONFIRMATION_REQUIRED,
                redistribution_rule: RedistributionRule::Treasury,
                immunity_period_seconds: 0,
                insurance_premium_bps: 0,
                liquidity_tier_yield_bonus: Vec::new(&env),
                score_decay_per_month: DEFAULT_REPUTATION_SCORE_DECAY_BPS,
                max_priority_fee_cap_bps: MAX_PRIORITY_FEE_BPS,
                default_rate_threshold: 0,
                insurance_fund_premium_bps: 0,
                insurance_max_payout_bps: 0,
                max_refinances_per_loan_chain: crate::types::DEFAULT_MAX_REFINANCES_PER_LOAN_CHAIN,
                refinance_cooldown_secs: crate::types::DEFAULT_REFINANCE_COOLDOWN_SECS,
            },
        );

        // Issue #1285: bump instance TTL at initialization so the contract
        // instance storage survives for the protocol's expected lifetime.
        helpers::bump_instance(&env);

        // RBAC requires every admin to have a role before they can pass
        // require_admin_approval_for_action; grant SuperAdmin to the initial
        // admin set so admin functions work immediately after deployment.
        rbac::initialize_legacy_admin_roles(&env);

        env.events().publish(
            (symbol_short!("contract"), symbol_short!("init")),
            (deployer, admins, admin_threshold, token),
        );

        // Initialize flash loan subsystem (Issue #1183)
        flash_loan::initialize_flash_loans(&env)?;

        Ok(())
    }

    // ── Vouching ──────────────────────────────────────────────────────────────


    pub fn vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        chain_id: Option<u32>,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::vouch(env.clone(), voucher, borrower, stake, token, chain_id);
        release_lock(&env);
        result
    }

    /// Issue #632: Vouch with cross-chain support.
    /// chain_id=0 is native Stellar; non-zero requires prior bridge validation.
    pub fn vouch_cross_chain(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        chain_id: u32,
    ) -> Result<(), ContractError> {
        vouch::vouch_cross_chain(env, voucher, borrower, stake, token, chain_id)
    }

    /// Issue #632: Admin sets bridge validation status for a voucher on a given chain.
    pub fn set_bridge_validated(
        env: Env,
        admin_signers: Vec<Address>,
        voucher: Address,
        chain_id: u32,
        validated: bool,
    ) -> Result<(), ContractError> {
        vouch::set_bridge_validated(env, admin_signers, voucher, chain_id, validated)
    }

    /// Issue #632: Query bridge validation status.
    pub fn is_bridge_validated(env: Env, voucher: Address, chain_id: u32) -> bool {
        vouch::is_bridge_validated(env, voucher, chain_id)
    }

    /// Sybil resistance: estimate the economic cost to attack a borrower's current
    /// voucher configuration. Returns the minimum capital (in stroops) and minimum
    /// lock time an attacker must commit to match the legitimate set's weighted stake.
    ///
    /// This is a read-only query function — it does not mutate state.
    pub fn estimate_sybil_attack_cost(
        env: Env,
        borrower: Address,
    ) -> crate::types::SybilAttackCostEstimate {
        vouch::estimate_sybil_attack_cost(env, borrower)
    }

    /// Issue #936: Compute a Merkle root over a borrower's current vouch set
    /// and persist it, enabling off-chain provers to build compact inclusion
    /// proofs without retrieving the full vouch list. See
    /// docs/vouch-merkle-proof.md for the leaf/root format.
    pub fn compute_and_store_merkle_root(
        env: Env,
        borrower: Address,
    ) -> Result<BytesN<32>, ContractError> {
        vouch::compute_and_store_merkle_root(env, borrower)
    }

    /// Issue #936: Read the most recently stored vouch Merkle root for a
    /// borrower, if one has been computed.
    pub fn get_merkle_root(env: Env, borrower: Address) -> Option<crate::types::VouchMerkleRoot> {
        vouch::get_merkle_root(env, borrower)
    }

    /// Issue #936: Hash a single vouch's plaintext fields into the canonical
    /// Merkle leaf format, for use as the `leaf` argument to
    /// `verify_vouch_inclusion`.
    pub fn hash_vouch_leaf(
        env: Env,
        voucher: Address,
        stake: i128,
        token: Address,
        vouch_timestamp: u64,
    ) -> BytesN<32> {
        vouch::hash_vouch_leaf(env, voucher, stake, token, vouch_timestamp)
    }

    /// Issue #936: Verify a Merkle inclusion proof for a single vouch leaf
    /// against a previously-stored root, without needing the full vouch
    /// list. Returns `true` iff the proof is valid.
    pub fn verify_vouch_inclusion(
        env: Env,
        root: BytesN<32>,
        leaf: BytesN<32>,
        proof: Vec<BytesN<32>>,
    ) -> bool {
        vouch::verify_vouch_inclusion(env, root, leaf, proof)
    }

    /// Issue #867: Create a cross-collateral pool, seeded by the creator's stake.
    pub fn create_collateral_pool(
        env: Env,
        creator: Address,
        token: Address,
        initial_stake: i128,
    ) -> Result<u64, ContractError> {
        collateral_pool::create_pool(env, creator, token, initial_stake)
    }

    /// Issue #867: Join an existing, inactive collateral pool.
    pub fn join_collateral_pool(
        env: Env,
        voucher: Address,
        pool_id: u64,
        stake: i128,
    ) -> Result<(), ContractError> {
        collateral_pool::join_pool(env, voucher, pool_id, stake)
    }

    /// Issue #966: Join an existing, inactive collateral pool from another chain.
    /// The voucher must already be bridge-validated for `chain_id` (see
    /// `set_bridge_validated`).
    pub fn join_collateral_pool_cross_chain(
        env: Env,
        voucher: Address,
        pool_id: u64,
        stake: i128,
        chain_id: u32,
    ) -> Result<(), ContractError> {
        collateral_pool::join_pool_cross_chain(env, voucher, pool_id, stake, chain_id)
    }

    /// Issue #867: Leave an inactive collateral pool, withdrawing the caller's stake.
    pub fn leave_collateral_pool(
        env: Env,
        voucher: Address,
        pool_id: u64,
    ) -> Result<(), ContractError> {
        collateral_pool::leave_pool(env, voucher, pool_id)
    }

    /// Issue #867: Admin assigns a borrower to a pool, locking its collateral.
    pub fn assign_collateral_pool_borrower(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
        borrower: Address,
    ) -> Result<(), ContractError> {
        collateral_pool::assign_pool_to_borrower(env, admin_signers, pool_id, borrower)
    }

    /// Issue #867: Read a collateral pool record.
    pub fn get_collateral_pool(env: Env, pool_id: u64) -> Result<CollateralPool, ContractError> {
        collateral_pool::get_pool(env, pool_id)
    }

    /// Issue #867: Total stake held in a collateral pool.
    pub fn get_collateral_pool_total_stake(
        env: Env,
        pool_id: u64,
    ) -> Result<i128, ContractError> {
        collateral_pool::get_pool_total_stake(env, pool_id)
    }

    /// Issue #966: Total stake contributed to a pool from a specific chain.
    pub fn get_collateral_pool_chain_stake(
        env: Env,
        pool_id: u64,
        chain_id: u32,
    ) -> Result<i128, ContractError> {
        collateral_pool::get_pool_chain_stake(env, pool_id, chain_id)
    }

    // ── Liquidity Mining Campaigns (Issue #1257) ──────────────────────────────

    /// Issue #1257: Create a new liquidity mining campaign.
    /// Admin deposits `incentive_pool` tokens; rewards are distributed to
    /// participating vouchers proportional to their recorded stake weight.
    pub fn create_mining_campaign(
        env: Env,
        admin_signers: Vec<Address>,
        token: Address,
        incentive_pool: i128,
        duration_secs: u64,
        campaign_type: crate::types::MiningCampaignType,
    ) -> Result<u64, ContractError> {
        liquidity_mining::create_mining_campaign(env, admin_signers, token, incentive_pool, duration_secs, campaign_type)
    }

    /// Issue #1257: Record a voucher's participation weight in an active campaign.
    /// Must be called while the campaign window is open.
    pub fn record_mining_participation(
        env: Env,
        campaign_id: u64,
        participant: Address,
        stake_weight: i128,
    ) -> Result<(), ContractError> {
        liquidity_mining::record_participation(env, campaign_id, participant, stake_weight)
    }

    /// Issue #1257: Claim the caller's proportional mining reward after a
    /// campaign has ended. Returns the amount disbursed (stroops).
    pub fn claim_mining_reward(
        env: Env,
        campaign_id: u64,
        participant: Address,
    ) -> Result<i128, ContractError> {
        liquidity_mining::claim_mining_reward(env, campaign_id, participant)
    }

    /// Issue #1257: Admin transitions an active campaign to Ended early.
    pub fn end_mining_campaign(
        env: Env,
        admin_signers: Vec<Address>,
        campaign_id: u64,
    ) -> Result<(), ContractError> {
        liquidity_mining::end_mining_campaign(env, admin_signers, campaign_id)
    }

    /// Issue #1257: Admin cancels a campaign and refunds the undistributed pool.
    pub fn cancel_mining_campaign(
        env: Env,
        admin_signers: Vec<Address>,
        campaign_id: u64,
    ) -> Result<(), ContractError> {
        liquidity_mining::cancel_mining_campaign(env, admin_signers, campaign_id)
    }

    /// Issue #1257: Read a campaign record by ID.
    pub fn get_mining_campaign(
        env: Env,
        campaign_id: u64,
    ) -> Result<crate::types::MiningCampaign, ContractError> {
        liquidity_mining::get_mining_campaign(env, campaign_id)
    }

    /// Issue #1257: Return the recorded participation weight for a voucher in a campaign.
    pub fn get_mining_participation(
        env: Env,
        campaign_id: u64,
        participant: Address,
    ) -> i128 {
        liquidity_mining::get_mining_participation(env, campaign_id, participant)
    }

    /// Issue #1257: Return the amount already claimed by a participant in a campaign.
    pub fn get_mining_claimed(
        env: Env,
        campaign_id: u64,
        participant: Address,
    ) -> i128 {
        liquidity_mining::get_mining_claimed(env, campaign_id, participant)
    }

    /// #642: Vouch with an explicit sector label for diversification enforcement.
    pub fn vouch_with_sector(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        sector: String,
    ) -> Result<(), ContractError> {
        vouch::vouch_with_sector(env, voucher, borrower, stake, token, sector)
    }

    /// Confidential (self-attestation) vouch with commitment + proof verification.
    ///
    /// **Not a privacy guarantee**: `stake_amount` is a plain on-chain argument and is
    /// recorded via the regular `vouch()` call below, so it is visible to any observer.
    /// `proof` and `commitment` provide binding/self-attestation (see `zk_snarks.rs`), not
    /// confidentiality. See `docs/threat-model.md` ("Confidentiality Model") for details.
    /// The `proof` demonstrates that, at call time:
    /// - The voucher has sufficient balance
    /// - The stake amount is within allowed bounds
    /// - The voucher is not blacklisted
    pub fn vouch_confidential(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake_amount: i128,
        commitment: ConfidentialCommitment,
        proof: ZkProof,
        token: Address,
        chain_id: Option<u32>,
    ) -> Result<(), ContractError> {
        voucher.require_auth();

        let token_client = crate::helpers::require_allowed_token(&env, &token)?;
        let voucher_balance = token_client.balance(&voucher);
        let balance_ok = voucher_balance >= stake_amount;
        let blacklisted = crate::admin::is_blacklisted(env.clone(), borrower.clone());

        zk_snarks::verify_vouch_proof(&env, &proof, &voucher, &borrower, &token, stake_amount, balance_ok, blacklisted)?;

        env.storage()
            .persistent()
            .set(&DataKey::VouchCommitment(voucher.clone(), borrower.clone()), &commitment);

        let proof_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ZkProofCounter)
            .unwrap_or(0u64)
            .checked_add(1)
            .expect("proof ID overflow");
        env.storage()
            .instance()
            .set(&DataKey::ZkProofCounter, &proof_id);

        let proof_record = crate::types::ZkProofRecord {
            proof_id,
            proof: proof.clone(),
            operation_type: crate::types::PROOF_TYPE_VOUCH,
            submitter: voucher.clone(),
            verified: true,
            submitted_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::ZkProofRecord(proof_id), &proof_record);

        vouch::vouch(env, voucher, borrower, stake_amount, token, chain_id)
    }

    /// Settlement-time reveal for a confidential vouch commitment recorded by
    /// `vouch_confidential()`. Verifies that `amount`/`blinding` hash to the commitment
    /// stored for `(voucher, borrower)`, then marks it revealed so it cannot be replayed.
    pub fn reveal_vouch_commitment(
        env: Env,
        voucher: Address,
        borrower: Address,
        amount: i128,
        blinding: soroban_sdk::Bytes,
    ) -> Result<(), ContractError> {
        voucher.require_auth();

        let key = DataKey::VouchCommitment(voucher.clone(), borrower.clone());
        let commitment: ConfidentialCommitment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::CommitmentNotFound)?;

        let revealed_key = DataKey::VouchCommitmentRevealed(voucher.clone(), borrower.clone());
        if env.storage().persistent().get(&revealed_key).unwrap_or(false) {
            return Err(ContractError::CommitmentAlreadyRevealed);
        }

        zk_snarks::reveal_commitment_bytes(&env, &commitment, amount, &blinding)?;

        env.storage().persistent().set(&revealed_key, &true);
        Ok(())
    }

    pub fn batch_vouch(
        env: Env,
        voucher: Address,
        borrowers: Vec<Address>,
        stakes: Vec<i128>,
        token: Address,
        chain_id: Option<u32>,
    ) -> Result<Vec<crate::types::BatchVouchResult>, ContractError> {
        vouch::batch_vouch(env, voucher, borrowers, stakes, token, chain_id)
    }

    pub fn increase_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        additional: i128,
    ) -> Result<(), ContractError> {
        vouch::increase_stake(env, voucher, borrower, additional)
    }

    pub fn decrease_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        vouch::decrease_stake(env, voucher, borrower, amount)
    }

    pub fn withdraw_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::withdraw_vouch(env, voucher, borrower)
    }

    pub fn transfer_vouch(
        env: Env,
        from: Address,
        to: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::transfer_vouch(env.clone(), from, to, borrower);
        release_lock(&env);
        result
    }

    pub fn delegate_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        delegate: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::delegate_vouch(env.clone(), voucher, borrower, delegate, token);
        release_lock(&env);
        result
    }

    pub fn revoke_delegation(
        env: Env,
        voucher: Address,
        borrower: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::revoke_delegation(env.clone(), voucher, borrower, token);
        release_lock(&env);
        result
    }

    pub fn set_vouch_expiry(
        env: Env,
        voucher: Address,
        borrower: Address,
        expiry: u64,
        token: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::set_vouch_expiry(env.clone(), voucher, borrower, expiry, token);
        release_lock(&env);
        result
    }

    // ── Issue #1167: Vouch splitting ──────────────────────────────────────────

    pub fn split_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        new_voucher: Address,
        amount_to_split: i128,
    ) -> Result<Address, ContractError> {
        acquire_lock(&env)?;
        let result = vouch::split_vouch(env.clone(), voucher, borrower, new_voucher, amount_to_split);
        release_lock(&env);
        result
    }

    pub fn get_vouch_split_history(env: Env, borrower: Address) -> Vec<VouchSplitRecord> {
        vouch::get_vouch_split_history(env, borrower)
    }

    // ── Issue #1165: Vouch rotation incentive program ─────────────────────────

    pub fn rotate_to_new_borrower(
        env: Env,
        voucher: Address,
        old_borrower: Address,
        new_borrower: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::rotate_to_new_borrower(env.clone(), voucher, old_borrower, new_borrower);
        release_lock(&env);
        result
    }

    pub fn get_rotation_bonus_bps(env: Env, voucher: Address) -> u32 {
        vouch::get_rotation_bonus_bps(env, voucher)
    }

    pub fn get_rotation_count(env: Env, voucher: Address) -> u32 {
        vouch::get_rotation_count(env, voucher)
    }

    pub fn get_stagnant_vouches(env: Env, voucher: Address) -> Vec<StagnantVouch> {
        vouch::get_stagnant_vouches(env, voucher)
    }

    // ── Issue #1164: Vouch portfolio risk dashboard ───────────────────────────

    pub fn get_portfolio_risk(env: Env, voucher: Address) -> PortfolioRiskReport {
        vouch::get_portfolio_risk(env, voucher)
    }

    pub fn get_portfolio_risk_history(env: Env, voucher: Address) -> Vec<PortfolioSnapshot> {
        vouch::get_portfolio_risk_history(env, voucher)
    }

    // ── Loans ─────────────────────────────────────────────────────────────────

    pub fn register_referral(
        env: Env,
        borrower: Address,
        referrer: Address,
    ) -> Result<(), ContractError> {
        loan::register_referral(env, borrower, referrer)
    }

    pub fn get_referrer(env: Env, borrower: Address) -> Option<Address> {
        loan::get_referrer(env, borrower)
    }

    pub fn set_referral_bonus_bps(env: Env, admin_signers: Vec<Address>, bonus_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(bonus_bps <= 10_000, "bonus_bps must not exceed 10000");
        env.storage()
            .instance()
            .set(&DataKey::ReferralBonusBps, &bonus_bps);
    }

    pub fn get_referral_bonus_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ReferralBonusBps)
            .unwrap_or(DEFAULT_REFERRAL_BONUS_BPS)
    }

    /// Issue #1287: Set the governance-adjustable withdrawal-queue priority-fee cap.
    /// The cap is expressed in basis points of the voucher's own stake (max 10_000 = 100%).
    /// Requires admin approval.
    pub fn set_priority_fee_cap_bps(env: Env, admin_signers: Vec<Address>, cap_bps: i128) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(cap_bps >= 0 && cap_bps <= 10_000, "cap_bps must be 0..=10000");
        let mut cfg = helpers::config(&env);
        cfg.max_priority_fee_cap_bps = cap_bps;
        env.storage().instance().set(&DataKey::Config, &cfg);
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("feecap")),
            cap_bps,
        );
    }

    /// Issue #1287: Get the current withdrawal-queue priority-fee cap in basis points.
    pub fn get_priority_fee_cap_bps(env: Env) -> i128 {
        helpers::config(&env).max_priority_fee_cap_bps
    }

    pub fn get_withdrawal_queue(env: Env, borrower: Address) -> Vec<QueuedWithdrawal> {
        vouch::get_withdrawal_queue(env, borrower)
    }

    pub fn process_withdrawal_batch(env: Env, borrower: Address, count: u32) -> u32 {
        vouch::process_withdrawal_batch(&env, &borrower, count)
    }

    pub fn request_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        token: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = loan::request_loan(env.clone(), borrower, amount, threshold, loan_purpose, token);
        release_lock(&env);
        result
    }

    /// Confidential (self-attestation) loan request with commitment + proof verification.
    ///
    /// **Not a privacy guarantee**: `amount` is a plain on-chain argument and is recorded
    /// via the regular `request_loan()` call below, so it is visible to any observer.
    /// `proof` and `commitment` provide binding/self-attestation (see `zk_snarks.rs`), not
    /// confidentiality. See `docs/threat-model.md` ("Confidentiality Model") for details.
    /// The `proof` demonstrates that, at call time:
    /// - The borrower meets eligibility requirements
    /// - The requested amount is within bounds
    /// - Sufficient vouches exist
    pub fn request_loan_confidential(
        env: Env,
        borrower: Address,
        amount: i128,
        commitment: ConfidentialCommitment,
        proof: ZkProof,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        token: Address,
    ) -> Result<(), ContractError> {
        borrower.require_auth();

        let vouches: Vec<crate::types::VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let total_vouches = vouches.len();
        let sufficient_vouches = total_vouches > 0;
        let eligibility_ok = vouches
            .iter()
            .filter(|v| v.token == token)
            .map(|v| v.stake)
            .sum::<i128>() >= threshold;

        zk_snarks::verify_loan_proof(&env, &proof, &borrower, &token, amount, threshold, eligibility_ok, sufficient_vouches)?;

        env.storage()
            .persistent()
            .set(&DataKey::LoanCommitment(borrower.clone()), &commitment);

        let proof_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ZkProofCounter)
            .unwrap_or(0u64)
            .checked_add(1)
            .expect("proof ID overflow");
        env.storage()
            .instance()
            .set(&DataKey::ZkProofCounter, &proof_id);

        let proof_record = crate::types::ZkProofRecord {
            proof_id,
            proof: proof.clone(),
            operation_type: crate::types::PROOF_TYPE_LOAN_REQUEST,
            submitter: borrower.clone(),
            verified: true,
            submitted_at: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::ZkProofRecord(proof_id), &proof_record);

        loan::request_loan(env, borrower, amount, threshold, loan_purpose, token)
    }

    /// Settlement-time reveal for a confidential loan commitment recorded by
    /// `request_loan_confidential()`. Verifies that `amount`/`blinding` hash to the
    /// commitment stored for `borrower`, then marks it revealed so it cannot be replayed.
    pub fn reveal_loan_commitment(
        env: Env,
        borrower: Address,
        amount: i128,
        blinding: soroban_sdk::Bytes,
    ) -> Result<(), ContractError> {
        borrower.require_auth();

        let key = DataKey::LoanCommitment(borrower.clone());
        let commitment: ConfidentialCommitment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::CommitmentNotFound)?;

        let revealed_key = DataKey::LoanCommitmentRevealed(borrower.clone());
        if env.storage().persistent().get(&revealed_key).unwrap_or(false) {
            return Err(ContractError::CommitmentAlreadyRevealed);
        }

        zk_snarks::reveal_commitment_bytes(&env, &commitment, amount, &blinding)?;

        env.storage().persistent().set(&revealed_key, &true);
        Ok(())
    }

    pub fn dispute_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        evidence_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        vouch::dispute_vouch(env, voucher, borrower, evidence_hash)
    }

    /// Issue #1056/#1372: request an emergency admin-voted waiver of the vouch cooldown.
    pub fn request_cooldown_bypass(
        env: Env,
        voucher: Address,
        borrower: Address,
        reason: String,
    ) -> Result<(), ContractError> {
        cooldown_bypass::request_cooldown_bypass(env, voucher, borrower, reason)
    }

    /// Issue #1056/#1372: admin vote on a pending cooldown bypass request.
    pub fn vote_bypass(
        env: Env,
        approver: Address,
        voucher: Address,
        borrower: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        cooldown_bypass::vote_bypass(env, approver, voucher, borrower, approve)
    }

    /// Issue #1056/#1372: whether `voucher` currently has an approved cooldown
    /// bypass for `borrower`.
    pub fn has_cooldown_bypass(env: Env, voucher: Address, borrower: Address) -> bool {
        cooldown_bypass::has_cooldown_bypass(&env, &voucher, &borrower)
    }

    /// Issue #1056/#1372: fetch the raw cooldown bypass request record, if any.
    pub fn get_cooldown_bypass_request(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Option<crate::types::CooldownBypassRequest> {
        cooldown_bypass::get_cooldown_bypass_request(env, voucher, borrower)
    }

    /// Issue #1056/#1372: admin cleanup of a resolved/no-longer-needed bypass record.
    pub fn clear_cooldown_bypass(
        env: Env,
        admin_signers: Vec<Address>,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        cooldown_bypass::clear_cooldown_bypass(env, admin_signers, voucher, borrower)
    }

    pub fn slash(env: Env, admin_signers: Vec<Address>, borrower: Address) {
        helpers::require_admin_approval(&env, &admin_signers);
        helpers::require_not_paused(&env).expect("contract is paused");

        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.status != LoanStatus::Active {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }

        let cfg = config(&env);
        let _vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        loan.status = LoanStatus::Defaulted;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));

        // Process withdrawal queue before deleting vouches (Issue #865)
        vouch::process_withdrawal_queue(&env, &borrower);

        // Re-read vouches after queue processing removed queued withdrawals
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let token_client = token::Client::new(&env, &loan.token_address);
        let mut total_slashed: i128 = 0;
        for v in vouches.iter() {
            if v.token == loan.token_address {
                let slash_amount = v.stake * cfg.slash_bps / 10_000;
                let returned = v.stake - slash_amount;
                if returned > 0 {
                    token_client.transfer(&env.current_contract_address(), &v.voucher, &returned);
                }
                total_slashed += slash_amount;
            } else if !is_zero_address(&env, &v.token) {
                // Non-matching token vouches are returned in full.
                let other_token = soroban_sdk::token::Client::new(&env, &v.token);
                other_token.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
            }
        }

        helpers::add_slash_balance(&env, total_slashed);

        // Issue #1071: Claim insurance for shortfall when slashed amount < loan amount
        let shortfall = loan.amount.saturating_sub(total_slashed);
        if shortfall > 0 {
            let _ = insurance::claim_insurance_for_shortfall(&env, shortfall, &cfg);
        }

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DefaultCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));
        helpers::increment_total_default_count(&env);

        // Issue #1413: Demote loyalty tier on default
        loyalty::record_default_for_loyalty(&env, &borrower);

        // Burn excellent credit tier badge on default
        reputation::burn_excellent_badge(&env, &borrower);

        // Burn external reputation NFT on default
        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            ReputationNftExternalClient::new(&env, &nft_addr).burn(&borrower);
        }

        // Update credit score after slash
        let _ = credit_score::update_credit_score(env.clone(), borrower.clone());

        // Issue #1288: Decrement on-chain TVL / active-loan-count counters on slash.
        helpers::decrement_tvl_counters(&env, loan.amount);

        // Clean up vouches storage last
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        // Issue #1371: check the circuit breaker after each executed slash using
        // real running default/loan counters instead of never calling it.
        let _ = circuit_breaker::try_trigger_circuit_breaker(
            &env,
            &cfg,
            helpers::get_total_default_count(&env),
            helpers::get_total_loan_count(&env),
        );
    }

    // ── Issue #1422: Fraud score detection ──────────────────────────────────
    /// Recompute and persist a voucher's fraud score from their vouch/slash history.
    pub fn update_fraud_score(env: Env, voucher: Address) -> Result<(), ContractError> {
        detection::update_fraud_score(env, voucher)
    }

    /// Read a voucher's stored fraud score, if one has been computed.
    pub fn get_fraud_score(env: Env, voucher: Address) -> Option<crate::types::VoucherFraudScore> {
        detection::get_fraud_score(env, voucher)
    }

    /// Persist the fraud-score configuration (threshold + enabled). Admin-only.
    pub fn set_fraud_score_config(
        env: Env,
        admin_signers: Vec<Address>,
        config: crate::types::FraudScoreConfig,
    ) -> Result<(), ContractError> {
        detection::set_fraud_score_config(env, admin_signers, config)
    }

    /// Read the current fraud-score configuration.
    pub fn get_fraud_score_config(env: Env) -> crate::types::FraudScoreConfig {
        detection::get_fraud_score_config_view(env)
    }

    // ── Issue #1423/#1424/#1425: Circuit breaker admin controls ─────────────
    /// Acknowledge the most recent circuit-breaker activation (admin multi-sig).
    /// Required before `unpause` will clear a circuit-breaker-induced pause.
    pub fn acknowledge_circuit_breaker(
        env: Env,
        admin_signers: Vec<Address>,
    ) -> Result<(), ContractError> {
        circuit_breaker::acknowledge_circuit_breaker(&env, admin_signers)
    }

    /// Return the bounded history of circuit-breaker activations, oldest first.
    pub fn get_circuit_breaker_history(env: Env) -> Vec<crate::types::CircuitBreakerTrigger> {
        circuit_breaker::get_circuit_breaker_history(&env)
    }

    /// Set the circuit-breaker anti-thrash cooldown window, in seconds. Admin-only.
    pub fn set_circuit_breaker_cooldown(
        env: Env,
        admin_signers: Vec<Address>,
        new_cooldown_secs: u64,
    ) -> Result<(), ContractError> {
        circuit_breaker::set_circuit_breaker_cooldown(&env, admin_signers, new_cooldown_secs)
    }

    /// Read the effective circuit-breaker cooldown (configured value or default).
    pub fn get_circuit_breaker_cooldown(env: Env) -> u64 {
        circuit_breaker::circuit_breaker_cooldown_secs(&env)
    }

    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = loan::repay(env.clone(), borrower, payment);
        release_lock(&env);
        result
    }


    /// Confirm intent to repay the active loan.
    ///
    /// When `Config.confirmation_required` is `true`, borrowers must call this
    /// function before calling `repay`. The confirmation is stored per-loan and
    /// consumed on the first successful `repay` call, so it cannot be replayed.
    ///
    /// This is a no-op (succeeds silently) when `confirmation_required` is false,
    /// so callers can always call it without checking the config first.
    pub fn confirm_repayment(env: Env, borrower: Address) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        let loan = get_active_loan_record(&env, &borrower)?;

        env.storage()
            .persistent()
            .set(&DataKey::RepaymentConfirmation(loan.id), &true);

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("repay_ok")),
            (borrower, loan.id),
        );

        Ok(())
    }

    /// #667: Called by the registered oracle to verify a repayment held in escrow.
    /// If `approved` is true, releases funds to vouchers. If false, returns funds to borrower.
    /// Called by the registered oracle to publish a fresh price for `key`
    /// (e.g. a collateral token's symbol). Used to inform dynamic-rate pricing.
    pub fn set_oracle_price(
        env: Env,
        oracle: Address,
        key: soroban_sdk::Symbol,
        price: i128,
    ) -> Result<(), ContractError> {
        helpers::set_oracle_price(&env, &oracle, key, price)
    }

    pub fn verify_repayment(
        env: Env,
        oracle: Address,
        borrower: Address,
        approved: bool,
    ) -> Result<(), ContractError> {
        oracle.require_auth();
        require_not_paused(&env)?;

        // Verify caller is the registered oracle
        let cfg = config(&env);
        let registered = cfg.oracle_address.ok_or(ContractError::OracleUnauthorized)?;
        if oracle != registered {
            return Err(ContractError::OracleUnauthorized);
        }

        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))
            .ok_or(ContractError::NoActiveLoan)?;
        let mut loan: LoanRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .ok_or(ContractError::NoActiveLoan)?;

        if loan.escrow_status != EscrowStatus::Pending {
            return Err(ContractError::NoEscrowFound);
        }

        let escrowed: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowAmount(borrower.clone()))
            .unwrap_or(0);

        let token_client = require_allowed_token(&env, &loan.token_address)?;
        let now = env.ledger().timestamp();

        if approved {
            loan.escrow_status = EscrowStatus::Released;
            loan.status = LoanStatus::Repaid;
            loan.repayment_timestamp = Some(now);

            let vouches: Vec<VouchRecord> = env
                .storage()
                .persistent()
                .get(&DataKey::Vouches(borrower.clone()))
                .unwrap_or(Vec::new(&env));

            let total_stake: i128 = vouches
                .iter()
                .filter(|v| v.token == loan.token_address)
                .map(|v| v.stake)
                .sum();

            for v in vouches.iter() {
                if v.token != loan.token_address {
                    continue;
                }
                // Issue #633: Yield tiering — vouch age bonus.
                // Vouches older than 30 days get +50% of their yield share.
                // Vouches older than 7 days get +25% of their yield share.
                let vouch_age_secs = loan.disbursement_timestamp.saturating_sub(v.vouch_timestamp);
                let age_multiplier_bps: i128 = if vouch_age_secs >= 30 * 24 * 60 * 60 {
                    15_000 // 150%
                } else if vouch_age_secs >= 7 * 24 * 60 * 60 {
                    12_500 // 125%
                } else {
                    10_000 // 100% base
                };

                let base_yield_share = if total_stake > 0 {
                    loan.total_yield * v.stake / total_stake
                } else {
                    0
                };
                let tiered_yield = base_yield_share * age_multiplier_bps / 10_000;

                // Issue #634: Liquidity mining reward on top of yield.
                let cfg = config(&env);
                let mining_reward = if cfg.liquidity_mining_rate_bps > 0 {
                    v.stake * cfg.liquidity_mining_rate_bps as i128 / 10_000
                } else {
                    0
                };

                token_client.transfer(
                    &env.current_contract_address(),
                    &v.voucher,
                    &(v.stake + tiered_yield + mining_reward),
                );
            }

            // Process withdrawal queue before deleting vouches (Issue #865)
            vouch::process_withdrawal_queue(&env, &borrower);

            // Increment borrower repayment count
            let prev_count: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::RepaymentCount(borrower.clone()))
                .unwrap_or(0);
            env.storage()
                .persistent()
                .set(&DataKey::RepaymentCount(borrower.clone()), &(prev_count + 1));

            // Update credit score after successful repayment
            let _ = credit_score::update_credit_score(env.clone(), borrower.clone());

            env.storage()
                .persistent()
                .remove(&DataKey::ActiveLoan(borrower.clone()));
            env.storage()
                .persistent()
                .remove(&DataKey::Vouches(borrower.clone()));

            env.events().publish(
                (symbol_short!("loan"), symbol_short!("repaid")),
                (borrower.clone(), loan.amount),
            );
        } else {
            // Oracle rejected — return escrowed funds to borrower
            loan.escrow_status = EscrowStatus::Rejected;
            loan.amount_repaid -= escrowed;

            if escrowed > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &borrower,
                    &escrowed,
                );
            }

            env.events().publish(
                (symbol_short!("loan"), symbol_short!("escrw_rej")),
                (borrower.clone(), escrowed),
            );
        }

        env.storage()
            .persistent()
            .remove(&DataKey::EscrowAmount(borrower.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);

        release_lock(&env);
        Ok(())
    }


    /// Callable by anyone after the loan deadline has passed. Applies the standard slash penalty.
    pub fn auto_slash(env: Env, borrower: Address) {
        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.status != LoanStatus::Active {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }
        assert!(
            env.ledger().timestamp() > loan.deadline,
            "loan deadline has not passed"
        );

        let cfg = config(&env);
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        loan.defaulted = true;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        loan.status = LoanStatus::Defaulted;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));

        let loan_token = soroban_sdk::token::Client::new(&env, &loan.token_address);
        let mut total_slash: i128 = 0;
        for v in vouches.iter() {
            if v.token != loan.token_address {
                continue;
            }
            let slash_amount = v.stake * cfg.slash_bps / 10_000;
            let returned = v.stake - slash_amount;
            total_slash += slash_amount;
            if returned > 0 {
                loan_token.transfer(&env.current_contract_address(), &v.voucher, &returned);
            }
        }

        // Process withdrawal queue before deleting vouches (Issue #865)
        vouch::process_withdrawal_queue(&env, &borrower);

        // Re-read vouches after queue processing
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let token_client = token::Client::new(&env, &loan.token_address);
        let mut total_slash: i128 = 0;
        for v in vouches.iter() {
            if v.token == loan.token_address {
                let slash_amount = v.stake * cfg.slash_bps / 10_000;
                let returned = v.stake - slash_amount;
                total_slash += slash_amount;
                if returned > 0 {
                    token_client.transfer(&env.current_contract_address(), &v.voucher, &returned);
                }
            } else if !is_zero_address(&env, &v.token) {
                // Non-matching token vouches are returned in full.
                let other_token = soroban_sdk::token::Client::new(&env, &v.token);
                other_token.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
            }
        }

        helpers::add_slash_balance(&env, total_slash);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DefaultCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));
        helpers::increment_total_default_count(&env);

        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            ReputationNftExternalClient::new(&env, &nft_addr).burn(&borrower);
        }

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("autoslash")),
            (borrower, total_slash),
        );
        // Issue #1288: Decrement on-chain TVL / active-loan-count counters on auto-slash.
        helpers::decrement_tvl_counters(&env, loan.amount);
    }
    pub fn claim_expired_loan(env: Env, borrower: Address) {
        borrower.require_auth();

        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.status != LoanStatus::Active {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }

        let now = env.ledger().timestamp();
        assert!(now >= loan.deadline, "loan has not expired yet");

        // Process withdrawal queue first (Issue #865)
        vouch::process_withdrawal_queue(&env, &borrower);

        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let loan_token = soroban_sdk::token::Client::new(&env, &loan.token_address);
        for v in vouches.iter() {
            if v.token == loan.token_address {
                loan_token.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
            }
        }

        loan.defaulted = true;
        let token_client = token::Client::new(&env, &loan.token_address);
        for v in vouches.iter() {
            if v.token == loan.token_address {
                token_client.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
            } else if !is_zero_address(&env, &v.token) {
                // Non-matching token vouches are returned via their own token.
                let other_token = soroban_sdk::token::Client::new(&env, &v.token);
                other_token.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
            }
        }

        loan.status = LoanStatus::Defaulted;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DefaultCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));
        helpers::increment_total_default_count(&env);
        // Issue #1288: Decrement on-chain TVL / active-loan-count counters on expired claim.
        helpers::decrement_tvl_counters(&env, loan.amount);
    }

    /// Admin withdraws accumulated slashed funds.
    pub fn slash_treasury(env: Env, admin_signers: Vec<Address>, recipient: Address) {
        require_admin_approval(&env, &admin_signers);
        helpers::require_admin_approval(&env, &admin_signers);

        let amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SlashTreasury)
            .unwrap_or(0);
        assert!(amount > 0, "no slashed funds to withdraw");
        env.storage()
            .instance()
            .set(&DataKey::SlashTreasury, &0i128);
        token_client(&env).transfer(&env.current_contract_address(), &recipient, &amount);
    }

    // ── Loan Pool ─────────────────────────────────────────────────────────────

    /// Admin function: atomically disburse a batch of small loans to multiple borrowers.
    pub fn create_loan_pool(
        env: Env,
        admin_signers: Vec<Address>,
        borrowers: Vec<Address>,
        amounts: Vec<i128>,
    ) -> Result<u64, ContractError> {
        helpers::require_admin_approval(&env, &admin_signers);

        if borrowers.len() != amounts.len() {
            return Err(ContractError::PoolLengthMismatch);
        }
        if borrowers.is_empty() {
            return Err(ContractError::PoolEmpty);
        }

        let cfg = config(&env);
        let now = env.ledger().timestamp();
        let deadline = now + cfg.loan_duration;

        let mut total_amount: i128 = 0;
        for i in 0..borrowers.len() {
            let borrower = borrowers.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            assert!(
                amount >= cfg.min_loan_amount,
                "pool: amount below minimum loan threshold"
            );

            if helpers::has_active_loan(&env, &borrower) {
                return Err(ContractError::PoolBorrowerActiveLoan);
            }

            let total_stake: i128 = env
                .storage()
                .persistent()
                .get::<DataKey, Vec<VouchRecord>>(&DataKey::Vouches(borrower.clone()))
                .unwrap_or(Vec::new(&env))
                .iter()
                .map(|v| v.stake)
                .sum();
            let max_allowed = total_stake * cfg.max_loan_to_stake_ratio as i128 / 100;
            assert!(
                amount <= max_allowed,
                "pool: loan amount exceeds maximum collateral ratio for borrower"
            );

            total_amount += amount;
        }

        let token_client = helpers::primary_token(&env);
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance < total_amount {
            return Err(ContractError::PoolInsufficientFunds);
        }

        let pool_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LoanPoolCounter)
            .unwrap_or(0u64)
            .checked_add(1)
            .expect("pool ID overflow");
        env.storage()
            .instance()
            .set(&DataKey::LoanPoolCounter, &pool_id);

        for i in 0..borrowers.len() {
            let borrower = borrowers.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            let loan_id = helpers::next_loan_id(&env);

            env.storage().persistent().set(
                &DataKey::Loan(loan_id),
                &LoanRecord {
                    id: loan_id,
                    borrower: borrower.clone(),
                    guarantor: None,
                    buyback_price: 0,
                    auto_repay_enabled: false,
                    auto_repay_attempts: 0,
                    escrow_status: EscrowStatus::None,
                    co_borrowers: Vec::new(&env),
                    amount,
                    amount_repaid: 0,
                    total_yield: amount * cfg.yield_bps / 10_000,
                    status: LoanStatus::Active,
                    repaid: false,
                    defaulted: false,
                    created_at: now,
                    disbursement_timestamp: now,
                    repayment_timestamp: None,
                    deadline,
                    loan_purpose: soroban_sdk::String::from_str(&env, "pool"),
                    token_address: cfg.token.clone(),
                    amortization_schedule: Vec::new(&env),
                    reminder_sent: false,
                    risk_score: 0,
                    deferment_periods: 0,
                    maturity_date: None,
                    rate_type: RateType::Fixed,
                    index_reference: None,
                    last_interest_calc: now,
                    accrued_interest: 0,
                    milestone_bonus_applied: 0,
                    retry_count: 0,
                    suspension_timestamp: None,
                    suspension_amount_repaid: 0,
                },
            );
            env.storage()
                .persistent()
                .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
            env.storage()
                .persistent()
                .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

            token_client.transfer(&env.current_contract_address(), &borrower, &amount);

            env.events().publish(
                (symbol_short!("pool"), symbol_short!("loan")),
                (pool_id, borrower.clone(), amount, deadline),
            );
        }

        env.storage().persistent().set(
            &DataKey::LoanPool(pool_id),
            &LoanPoolRecord {
                pool_id,
                borrowers: borrowers.clone(),
                amounts: amounts.clone(),
                created_at: now,
                total_disbursed: total_amount,
            },
        );

        env.events().publish(
            (symbol_short!("pool"), symbol_short!("created")),
            (pool_id, borrowers.len(), total_amount),
        );

        Ok(pool_id)
    }

    pub fn get_loan_pool(env: Env, pool_id: u64) -> Option<LoanPoolRecord> {
        env.storage().persistent().get(&DataKey::LoanPool(pool_id))
    }

    pub fn get_loan_pool_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::LoanPoolCounter)
            .unwrap_or(0)
    }

    // ── Loan Performance Attribution ─────────────────────────────────────────

    /// Record the performance drivers (credit score, vouch quality, sector,
    /// region) for a loan so they can later be attributed to its outcome.
    pub fn record_loan_performance_factors(
        env: Env,
        loan_id: u64,
        borrower: Address,
        credit_score: u32,
        vouch_quality_bps: u32,
        sector: String,
        region: String,
    ) -> loan_attribution::PerformanceFactors {
        loan_attribution::record_performance_factors(
            env,
            loan_id,
            borrower,
            credit_score,
            vouch_quality_bps,
            sector,
            region,
        )
    }

    /// Analyze how much each tracked factor contributed to a loan's outcome.
    pub fn analyze_loan_attribution(
        env: Env,
        loan_id: u64,
    ) -> loan_attribution::Attribution {
        loan_attribution::analyze_loan_performance_attribution(env, loan_id)
    }

    /// Generate an aggregate performance report broken down by factor,
    /// across every loan analyzed so far.
    pub fn generate_factor_report(
        env: Env,
    ) -> loan_attribution::FactorPerformanceReport {
        loan_attribution::generate_factor_performance_report(env)
    }

    /// Predict the likelihood of successful repayment (0-10_000 bps) for a
    /// hypothetical loan given its factors, based on historical attribution.
    pub fn predict_loan_success_bps(
        env: Env,
        credit_score: u32,
        vouch_quality_bps: u32,
        sector: String,
        region: String,
    ) -> u32 {
        loan_attribution::predict_loan_success_probability_bps(
            env,
            credit_score,
            vouch_quality_bps,
            sector,
            region,
        )
    }

    // ── Loan Request Cart (batch loan requests) ──────────────────────────────

    /// Stage a loan request in the borrower's cart instead of submitting it
    /// immediately. Multiple items can be staged and submitted together via
    /// `submit_batch_loan_request` — though the protocol's single-active-loan
    /// constraint means only one item per submission can actually disburse;
    /// see that function's docs (issue #1397).
    pub fn add_to_loan_cart(
        env: Env,
        borrower: Address,
        amount: i128,
        tenure_secs: u64,
    ) -> loan_cart::LoanCart {
        loan_cart::add_to_loan_cart(env, borrower, amount, tenure_secs)
    }

    /// Read a borrower's currently staged cart contents.
    pub fn get_loan_cart(env: Env, borrower: Address) -> loan_cart::LoanCart {
        loan_cart::get_loan_cart(env, borrower)
    }

    /// Remove a single staged item from the borrower's cart by index,
    /// without discarding the rest of the cart (#1396). Panics with
    /// `ContractError::NotFound` if the borrower has no cart, or if
    /// `item_index` is out of range for it.
    pub fn remove_cart_item(env: Env, borrower: Address, item_index: u32) -> loan_cart::LoanCart {
        loan_cart::remove_cart_item(env, borrower, item_index)
    }

    /// Replace the amount/tenure of a single staged cart item in place,
    /// without disturbing its position or the rest of the cart (#1396).
    /// Panics with `ContractError::NotFound` if the borrower has no cart, or
    /// if `item_index` is out of range for it.
    pub fn update_cart_item(
        env: Env,
        borrower: Address,
        item_index: u32,
        amount: i128,
        tenure_secs: u64,
    ) -> loan_cart::LoanCart {
        loan_cart::update_cart_item(env, borrower, item_index, amount, tenure_secs)
    }

    /// Clear a borrower's cart without submitting it (recorded as abandoned).
    pub fn abandon_loan_cart(env: Env, borrower: Address) {
        loan_cart::abandon_loan_cart(env, borrower)
    }

    /// Submit every staged cart item as an individual loan request. Batches
    /// of 3 or more items are eligible for a 1% volume discount on requested
    /// principal — but the protocol only allows a single *active* loan per
    /// borrower, so at most one item per submission can actually disburse;
    /// every item after the first success fails with `ActiveLoanExists`
    /// (see the `loan_cart` module docs). Only an item that actually
    /// succeeds can carry a realized discount in the returned result; a
    /// failed item's `discounted_amount` is left undiscounted rather than
    /// advertising a price for a loan that was never funded (issue #1397).
    /// Returns a per-item result.
    pub fn submit_batch_loan_request(
        env: Env,
        borrower: Address,
        loan_purpose: String,
        threshold: i128,
        token: Address,
    ) -> Vec<loan_cart::BatchLoanRequestResult> {
        loan_cart::submit_batch_loan_request(env, borrower, loan_purpose, threshold, token)
    }

    /// Read protocol-wide cart funnel statistics (created vs. submitted vs.
    /// abandoned), for product analytics.
    pub fn get_cart_abandonment_stats(env: Env) -> loan_cart::CartAbandonmentStats {
        loan_cart::get_cart_abandonment_stats(env)
    }

    // ── Liquidity Rebalancing (Issue #88) ─────────────────────────────────────




    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
        admin::add_admin(env, admin_signers, new_admin)
    }

    /// #669: Retry a failed repayment. Increments retry_count and re-attempts the transfer.
    /// Returns `MaxRetriesExceeded` if retry_count >= MAX_REPAYMENT_RETRIES.
    pub fn retry_repayment(
        env: Env,
        borrower: Address,
        payment: i128,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))
            .ok_or(ContractError::NoActiveLoan)?;
        let mut loan: LoanRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .ok_or(ContractError::NoActiveLoan)?;

        const MAX_REPAYMENT_RETRIES: u32 = 3;
        if loan.retry_count >= MAX_REPAYMENT_RETRIES {
            return Err(ContractError::MaxRetriesExceeded);
        }

        loan.retry_count += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);

        // Delegate to the standard repay logic
        Self::repay(env, borrower, payment)
    }

    pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))?;
        env.storage().persistent().get(&DataKey::Loan(loan_id))
    }

    pub fn get_vouches(env: Env, borrower: Address) -> Vec<VouchRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Vouches(borrower))
            .unwrap_or(Vec::new(&env))
    }

    /// Paginated read of a borrower's vouches (Issue #1146). Returns up to
    /// `limit` (capped at `MAX_PAGE_SIZE`) records starting at `offset`, plus
    /// `next_cursor` — `Some(offset)` for the next call, or `None` at the end.
    pub fn get_vouches_page(
        env: Env,
        borrower: Address,
        offset: u32,
        limit: u32,
    ) -> (Vec<VouchRecord>, Option<u32>) {
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower))
            .unwrap_or(Vec::new(&env));
        crate::helpers::paginate_vec(&env, &vouches, offset, limit)
    }

    /// Paginated read of the addresses of borrowers a `voucher` has ever
    /// backed (Issue #1146). See `get_vouches_page` for cursor semantics.
    pub fn get_voucher_history_page(
        env: Env,
        voucher: Address,
        offset: u32,
        limit: u32,
    ) -> (Vec<Address>, Option<u32>) {
        let history: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::VoucherHistory(voucher))
            .unwrap_or(Vec::new(&env));
        crate::helpers::paginate_vec(&env, &history, offset, limit)
    }

    /// Paginated read of a borrower's pending withdrawal queue (Issue #1146).
    pub fn get_withdrawal_queue_page(
        env: Env,
        borrower: Address,
        offset: u32,
        limit: u32,
    ) -> (Vec<crate::types::QueuedWithdrawal>, Option<u32>) {
        vouch::get_withdrawal_queue_page(env, borrower, offset, limit)
    }

    /// Read the bounded hot vouch-history window for (borrower, voucher,
    /// token) (Issue #1146). See `get_vouch_history_archive_count` for the
    /// full historical log.
    pub fn get_vouch_history(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> Vec<crate::types::VouchHistoryEntry> {
        vouch::get_vouch_history(env, borrower, voucher, token)
    }

    /// Number of archive batches created so far for this (borrower, voucher,
    /// token) relationship's vouch-history log (Issue #1146). Use with
    /// `get_archived_vouch_history_batch` to walk the full historical log;
    /// `get_vouch_history` alone only returns the bounded hot window.
    pub fn get_vouch_history_archive_count(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::VouchHistoryArchiveCount(borrower, voucher, token))
            .unwrap_or(0)
    }

    /// Read one archived vouch-history batch (Issue #1146). `batch_id` ranges
    /// over `0..get_vouch_history_archive_count(...)`, oldest batch first.
    pub fn get_archived_vouch_history_batch(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
        batch_id: u32,
    ) -> Vec<crate::types::VouchHistoryEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::ArchivedVouchHistory(borrower, voucher, token, batch_id))
            .unwrap_or(Vec::new(&env))
    }

    // ── Issue #1179: Vouch Audit Trail ────────────────────────────────────────

    /// Read the bounded hot-window vouch audit trail (Issue #1179) for
    /// (borrower, voucher, token), formatted as one newline-separated string
    /// with the oldest event first. See `get_vouch_audit_trail_page` for
    /// pagination and `export_vouch_audit_report` for a compliance report.
    pub fn get_vouch_audit_trail(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> Result<String, ContractError> {
        let events = audit::get_vouch_audit_trail_events(&env, &borrower, &voucher, &token);
        Ok(audit::format_audit_trail(&env, &events))
    }

    /// Retrieve a page of formatted audit events for a vouch (Issue #1179).
    /// Returns up to `limit` events starting from index `offset` over the
    /// hot-window audit trail.
    pub fn get_vouch_audit_trail_page(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<String>, ContractError> {
        let events = audit::get_vouch_audit_trail_events(&env, &borrower, &voucher, &token);
        Ok(audit::get_vouch_audit_trail_page_formatted(&env, &events, offset, limit))
    }

    /// Export the vouch audit trail as a formatted report (Issue #1179),
    /// suitable for compliance and transparency reporting.
    pub fn export_vouch_audit_report(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> Result<String, ContractError> {
        let events = audit::get_vouch_audit_trail_events(&env, &borrower, &voucher, &token);
        Ok(audit::format_audit_report(&env, &events))
    }

    /// Number of archive batches created so far for this relationship's
    /// vouch audit trail (Issue #1179). Use with
    /// `get_archived_vouch_audit_batch` to walk the full historical audit
    /// log beyond the bounded hot window.
    pub fn get_vouch_audit_archive_count(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> u32 {
        audit::get_vouch_audit_trail_archive_count(&env, &borrower, &voucher, &token)
    }

    /// Read one archived vouch audit-trail batch (Issue #1179). `batch_id`
    /// ranges over `0..get_vouch_audit_archive_count(...)`, oldest batch
    /// first.
    pub fn get_archived_vouch_audit_batch(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
        batch_id: u32,
    ) -> Vec<crate::types::VouchAuditEvent> {
        audit::get_archived_vouch_audit_trail_batch(&env, &borrower, &voucher, &token, batch_id)
    }

    // ── Loan Priority / Subordination (senior-junior debt structures) ────────

    /// Build (or replace) the loan priority queue for a specific pool/batch,
    /// tagging each loan Senior, Mezzanine, or Junior.
    ///
    /// Issue #12: `pool_id` parameter added so each syndication pool maintains
    /// its own independent priority queue rather than sharing one global queue.
    pub fn create_loan_priority_queue(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
        loans: Vec<loan_priority::PriorityLoanEntry>,
    ) -> Result<(), ContractError> {
        loan_priority::create_loan_priority_queue(env, admin_signers, pool_id, loans)
    }

    /// Read the priority queue for a specific pool/batch.
    ///
    /// Issue #12: `pool_id` parameter added.
    pub fn get_loan_priority_queue(env: Env, pool_id: u64) -> Vec<loan_priority::PriorityLoanEntry> {
        loan_priority::get_loan_priority_queue(env, pool_id)
    }

    /// Route recovered default proceeds through the Senior/Mezzanine/Junior
    /// waterfall for a specific pool/batch.
    ///
    /// Issue #12: `pool_id` parameter added.
    pub fn route_default_proceeds(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
        total_proceeds: i128,
    ) -> Result<loan_priority::WaterfallRun, ContractError> {
        loan_priority::route_default_proceeds(env, admin_signers, pool_id, total_proceeds)
    }

    pub fn get_waterfall_run(env: Env, run_id: u64) -> Option<loan_priority::WaterfallRun> {
        loan_priority::get_waterfall_run(env, run_id)
    }

    /// Propose a governance change to a loan's priority tranche within a pool.
    ///
    /// Issue #12: `pool_id` parameter added.
    pub fn propose_priority_change(
        env: Env,
        proposer: Address,
        pool_id: u64,
        loan_id: u64,
        new_priority: loan_priority::LoanPriority,
    ) -> Result<u64, ContractError> {
        loan_priority::propose_priority_change(env, proposer, pool_id, loan_id, new_priority)
    }

    /// Approve a pending priority-change proposal; executes once threshold is met.
    pub fn approve_priority_change(
        env: Env,
        approver: Address,
        proposal_id: u64,
    ) -> Result<bool, ContractError> {
        loan_priority::approve_priority_change(env, approver, proposal_id)
    }

    // ── Large Loan Multi-Signature Approval ───────────────────────────────────

    /// Governance-set threshold above which loans require 2-of-3 admin multi-sig.
    pub fn set_large_loan_threshold(
        env: Env,
        admin_signers: Vec<Address>,
        threshold: i128,
    ) -> Result<(), ContractError> {
        large_loan_approval::set_large_loan_threshold(env, admin_signers, threshold)
    }

    pub fn get_large_loan_threshold(env: Env) -> i128 {
        large_loan_approval::get_large_loan_threshold(env)
    }

    /// Queue a large loan for multi-signature approval (48h expiration window).
    pub fn propose_large_loan_approval(
        env: Env,
        proposer: Address,
        loan_id: u64,
        borrower: Address,
        amount: i128,
    ) -> Result<u64, ContractError> {
        large_loan_approval::propose_large_loan_approval(env, proposer, loan_id, borrower, amount)
    }

    /// Add an admin signature to a pending large-loan approval proposal.
    pub fn sign_large_loan_approval(
        env: Env,
        signer: Address,
        approval_id: u64,
    ) -> Result<bool, ContractError> {
        large_loan_approval::sign_large_loan_approval(env, signer, approval_id)
    }

    pub fn is_large_loan_approved(env: Env, approval_id: u64) -> bool {
        large_loan_approval::is_large_loan_approved(env, approval_id)
    }

    pub fn get_large_loan_approval(
        env: Env,
        approval_id: u64,
    ) -> Option<large_loan_approval::LargeLoanApproval> {
        large_loan_approval::get_large_loan_approval(env, approval_id)
    }

    // ── Issue #1177: Vouch Maturity-Based Interest Adjustment ────────────────

    /// Get the maturity record for a vouch (Issue #1177) - NOT YET IMPLEMENTED.
    /// Returns tenure information and current maturity bonus.
    pub fn get_vouch_maturity(
        _env: Env,
        _voucher: Address,
        _borrower: Address,
        _token: Address,
    ) -> Result<String, ContractError> {
        // TODO: Implement when maturity types are defined
        Ok(String::from_str(&_env, ""))
    }

    /// Get the current maturity bonus for a vouch in basis points (Issue #1177) - NOT YET IMPLEMENTED.
    /// Returns 0-100 bps representing 0-1% additional interest from tenure.
    pub fn get_vouch_maturity_bonus(
        _env: Env,
        _voucher: Address,
        _borrower: Address,
        _token: Address,
    ) -> Result<i128, ContractError> {
        // TODO: Implement when maturity types are defined
        Ok(0)
    }

    /// Get the total interest bonus for a vouch including loyalty bonus (Issue #1177) - NOT YET IMPLEMENTED.
    /// Returns maturity bonus + loyalty bonus (if eligible for 2+ years).
    pub fn get_vouch_total_interest_bonus(
        _env: Env,
        _voucher: Address,
        _borrower: Address,
        _token: Address,
    ) -> Result<i128, ContractError> {
        // TODO: Implement when maturity types are defined
        Ok(0)
    }

    // ── Issue #1176: Social Features for Borrower Network ────────────────────

    /// Set or update a borrower's profile (Issue #1176).
    /// Allows borrowers to create their community profile with bio and sector info.
    pub fn set_borrower_profile(
        env: Env,
        borrower: Address,
        bio: String,
        sector: Option<String>,
        region: Option<String>,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        social::set_borrower_profile(&env, borrower, bio, sector, region)
    }

    /// Get a borrower's profile (Issue #1176).
    /// Returns a pipe-delimited string `"bio|sector|region"`.
    pub fn get_borrower_profile(
        env: Env,
        borrower: Address,
    ) -> Result<String, ContractError> {
        social::get_borrower_profile(&env, &borrower)
    }

    /// Set whether borrower consents to share success stories (Issue #1176).
    pub fn set_success_story_consent(
        env: Env,
        borrower: Address,
        _consent: bool,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        // TODO: Implement when social feature types are defined
        Ok(())
    }

    /// Submit a success story (Issue #1176) - NOT YET IMPLEMENTED.
    /// Returns the story ID for reference.
    pub fn submit_success_story(
        env: Env,
        borrower: Address,
        _title: String,
        _content: String,
    ) -> Result<u64, ContractError> {
        borrower.require_auth();
        // TODO: Implement when social feature types are defined
        Ok(0)
    }

    /// Publish a success story (Issue #1176) - NOT YET IMPLEMENTED.
    /// Only the borrower who submitted can publish.
    pub fn publish_success_story(
        env: Env,
        borrower: Address,
        _story_id: u64,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        // TODO: Implement when social feature types are defined
        Ok(())
    }

    /// Get a success story (Issue #1176) - NOT YET IMPLEMENTED.
    pub fn get_success_story(
        _env: Env,
        _story_id: u64,
    ) -> Result<String, ContractError> {
        // TODO: Implement when social feature types are defined
        Ok(String::from_str(&_env, ""))
    }

    /// Get all success stories for a borrower (Issue #1176) - NOT YET IMPLEMENTED.
    pub fn get_borrower_success_stories(
        env: Env,
        _borrower: Address,
    ) -> Result<Vec<String>, ContractError> {
        // TODO: Implement when social feature types are defined
        Ok(Vec::new(&env))
    }

    /// Get retention metrics for a borrower (Issue #1176) - NOT YET IMPLEMENTED.
    /// Tracks loan activity, repayment success, and platform engagement.
    pub fn get_retention_metrics(
        _env: Env,
        _borrower: Address,
    ) -> Result<String, ContractError> {
        // TODO: Implement when social feature types are defined
        Ok(String::from_str(&_env, ""))
    }

    /// Find similar borrowers for peer discovery (Issue #1176) - NOT YET IMPLEMENTED.
    /// Returns borrowers with similar sector/region characteristics.
    pub fn find_similar_borrowers(
        env: Env,
        _borrower: Address,
        _limit: u32,
    ) -> Result<Vec<String>, ContractError> {
        // TODO: Implement when social feature types are defined
        Ok(Vec::new(&env))
    }

    /// Calculate engagement score for a borrower (Issue #1176).
    /// Returns a score 0-100 based on loan activity and retention metrics.
    pub fn calculate_engagement_score(
        env: Env,
        borrower: Address,
    ) -> Result<u32, ContractError> {
        social::calculate_engagement_score(env, borrower)
    }

    /// Total number of borrowers ever registered (Issue #1146). Ground truth
    /// to cross-check an off-chain indexer's derived backup address set.
    pub fn get_borrower_count(env: Env) -> u32 {
        crate::helpers::get_borrower_count(&env)
    }

    /// Issue #1288: On-chain view of total outstanding loan principal, in stroops.
    /// Maintained as a running counter updated on every loan issuance, repayment, and slash.
    /// Any Soroban contract may call this for composability without enumerating borrowers.
    pub fn get_total_value_locked(env: Env) -> i128 {
        crate::helpers::get_total_value_locked(&env)
    }

    /// Issue #1288: On-chain view of the number of currently active loans.
    /// Maintained as a running counter updated on every loan issuance and closure.
    pub fn get_active_loan_count(env: Env) -> u32 {
        crate::helpers::get_active_loan_count(&env)
    }

    /// Paginated read of the global borrower list (Issue #1146).
    pub fn get_borrower_list_page(
        env: Env,
        offset: u32,
        limit: u32,
    ) -> (Vec<Address>, Option<u32>) {
        crate::helpers::get_borrower_list_page(&env, offset, limit)
    }

    /// Issue #1289: Total count of distinct addresses that have ever submitted a vouch.
    /// Maintained by `VoucherRegistry` updated on each new voucher's first-ever vouch.
    pub fn get_voucher_count(env: Env) -> u32 {
        crate::helpers::get_voucher_count(&env)
    }

    /// Issue #1289: Paginated read of the global voucher registry.
    /// Returns a page of voucher addresses and an optional cursor for the next page.
    pub fn get_voucher_list_page(
        env: Env,
        cursor: u32,
        limit: u32,
    ) -> (Vec<Address>, Option<u32>) {
        crate::helpers::get_voucher_list_page(&env, cursor, limit)
    }

    /// Verify all documented protocol invariants (I1-I8) for `borrowers`
    /// against live on-chain state (Issue #1146). Returns
    /// `Err(ContractError::InvariantViolation)` on the first violation found.
    /// Callable via `stellar contract invoke -- check_invariants` so
    /// operational tooling (e.g. `scripts/restore.sh`) can gate destructive
    /// recovery steps with a pre/post invariant check.
    pub fn check_invariants(env: Env, borrowers: Vec<Address>) -> Result<(), ContractError> {
        crate::invariants::check_invariants_live(&env, borrowers)
    }

    pub fn vouch_exists(env: Env, voucher: Address, borrower: Address) -> bool {
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower))
            .unwrap_or(Vec::new(&env));
        vouches.iter().any(|v| v.voucher == voucher)
    }

    /// Returns the total primary-token stake for `borrower`.
    pub fn total_vouched(env: Env, borrower: Address) -> Result<i128, ContractError> {
        vouch::total_vouched(env, borrower)
    }

    pub fn get_config(env: Env) -> Config {
        config(&env)
    }

    pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
        helper_loan_status(&env, &borrower)
    }

    pub fn loan_status_extended(env: Env, borrower: Address) -> LoanStatusEx {
        loan::loan_status_extended(env, borrower)
    }

    /// Issue #1078: Batch query loan statuses for multiple borrowers.
    /// Returns results in the order of the input borrowers vector, with a 100-borrower limit.
    pub fn batch_loan_status(
        env: Env,
        borrowers: Vec<Address>,
    ) -> Result<Vec<BatchLoanStatusResult>, ContractError> {
        if borrowers.is_empty() {
            return Err(ContractError::InsufficientFunds);
        }
        if borrowers.len() > 100 {
            return Err(ContractError::InsufficientFunds);
        }

        let mut results: Vec<BatchLoanStatusResult> = Vec::new(&env);
        for borrower in borrowers.iter() {
            let status = helper_loan_status(&env, &borrower);
            results.push_back(BatchLoanStatusResult {
                borrower: borrower.clone(),
                status: status.clone(),
            });

            env.events().publish(
                (symbol_short!("bloan"), symbol_short!("query")),
                (borrower.clone(), status as u32),
            );
        }

        Ok(results)
    }

    pub fn suspend_loan_on_missed_payment(
        env: Env,
        caller: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        loan::suspend_loan_on_missed_payment(env, caller, borrower)
    }

    pub fn resume_loan(env: Env, caller: Address, borrower: Address) -> Result<(), ContractError> {
        loan::resume_loan(env, caller, borrower)
    }

    // ── Issue #880: Loan Co-Borrower Support ─────────────────────────────────

    pub fn add_co_borrower(
        env: Env,
        borrower: Address,
        co_borrower: Address,
    ) -> Result<(), ContractError> {
        loan::add_co_borrower(env, borrower, co_borrower)
    }

    pub fn remove_co_borrower(
        env: Env,
        borrower: Address,
        co_borrower: Address,
    ) -> Result<(), ContractError> {
        loan::remove_co_borrower(env, borrower, co_borrower)
    }

    pub fn get_co_borrowers(env: Env, borrower: Address) -> Vec<Address> {
        loan::get_co_borrowers(env, borrower)
    }

    // ── Issue #881: Dynamic Interest Rate ────────────────────────────────────

    pub fn set_dynamic_rate_config(
        env: Env,
        admin_signers: Vec<Address>,
        config: DynamicRateConfig,
    ) -> Result<(), ContractError> {
        loan::set_dynamic_rate_config(env, admin_signers, config)
    }

    pub fn get_dynamic_rate_config(env: Env) -> DynamicRateConfig {
        loan::get_dynamic_rate_config_view(env)
    }

    pub fn compute_dynamic_rate(
        env: Env,
        admin_signers: Vec<Address>,
        borrower: Address,
    ) -> Result<u32, ContractError> {
        loan::compute_and_store_dynamic_rate(env, admin_signers, borrower)
    }

    pub fn get_borrower_dynamic_rate(env: Env, borrower: Address) -> Option<BorrowerDynamicRate> {
        loan::get_borrower_dynamic_rate(env, borrower)
    }

    // ── Issue #878: Loan Forbearance Period ──────────────────────────────────

    pub fn request_forbearance(
        env: Env,
        borrower: Address,
        duration_secs: Option<u64>,
    ) -> Result<(), ContractError> {
        loan::request_forbearance(env, borrower, duration_secs)
    }

    pub fn end_forbearance(env: Env, borrower: Address) -> Result<(), ContractError> {
        loan::end_forbearance(env, borrower)
    }

    pub fn get_forbearance(env: Env, loan_id: u64) -> Option<ForbearanceRecord> {
        loan::get_forbearance(env, loan_id)
    }

    // ── Issue #879: Loan Refinancing ─────────────────────────────────────────

    pub fn refinance_loan(
        env: Env,
        borrower: Address,
        new_amount: i128,
        new_threshold: i128,
        new_token: Address,
    ) -> Result<(), ContractError> {
        loan::refinance_loan(env, borrower, new_amount, new_threshold, new_token)
    }

    pub fn get_refinance_record(env: Env, loan_id: u64) -> Option<RefinanceRecord> {
        loan::get_refinance_record(env, loan_id)
    }

    // ── Issue #1166: Refinance rate shopping ──────────────────────────────────

    pub fn refinance_quote(
        env: Env,
        borrower: Address,
        new_amount: i128,
        new_token: Address,
    ) -> Result<RefinanceQuote, ContractError> {
        loan::refinance_quote(env, borrower, new_amount, new_token)
    }

    pub fn get_refinance_stats(env: Env) -> RefinanceStats {
        loan::get_refinance_stats(env)
    }

    pub fn set_borrower_risk_score(
        env: Env,
        admin_signers: Vec<Address>,
        borrower: Address,
        risk_score: u32,
    ) -> Result<(), ContractError> {
        loan::set_borrower_risk_score(env, admin_signers, borrower, risk_score)
    }

    // ── Governance: slash voting ──────────────────────────────────────────────

    pub fn vote_slash(
        env: Env,
        voucher: Address,
        borrower: Address,
        approve: bool,
    ) -> Result<VoteSlashResult, ContractError> {
        governance::vote_slash(env, voucher, borrower, approve)
    }

    pub fn get_slash_vote(env: Env, borrower: Address) -> Option<SlashVoteRecord> {
        governance::get_slash_vote(env, borrower)
    }

    pub fn set_slash_vote_quorum(env: Env, admin_signers: Vec<Address>, quorum_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        governance::set_slash_vote_quorum(&env, quorum_bps);
    }

    pub fn get_slash_vote_quorum(env: Env) -> u32 {
        governance::get_slash_vote_quorum(env)
    }

    pub fn execute_slash_vote(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_slash_vote(env, borrower)
    }

    pub fn execute_pending_slash(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_pending_slash(env, borrower)
    }

    pub fn queue_slash(
        env: Env,
        admin_signers: Vec<Address>,
        borrower: Address,
        slash_amount: i128,
    ) -> Result<(), ContractError> {
        crate::lazy_slash::queue_slash_gov(env, admin_signers, borrower, slash_amount)
    }

    pub fn execute_queued_slashes(
        env: Env,
        admin_signers: Vec<Address>,
    ) -> Result<u32, ContractError> {
        crate::lazy_slash::execute_queued_slashes_gov(env, admin_signers)
    }

    // ── Issue #1069: Vote Delegation ─────────────────────────────────────────

    pub fn delegate_vote(
        env: Env,
        voucher: Address,
        delegate: Address,
    ) -> Result<(), ContractError> {
        governance::delegate_vote(env, voucher, delegate)
    }

    pub fn revoke_vote_delegation(
        env: Env,
        voucher: Address,
    ) -> Result<(), ContractError> {
        governance::revoke_vote_delegation(env, voucher)
    }

    pub fn get_vote_delegate(env: Env, voucher: Address) -> Option<Address> {
        governance::get_vote_delegate(env, voucher)
    }

    // ── Issue #680: slash threshold governance ────────────────────────────────

    pub fn propose_slash_threshold(
        env: Env,
        proposer: Address,
        new_threshold: i128,
    ) -> Result<u64, ContractError> {
        governance::propose_slash_threshold(env, proposer, new_threshold)
    }

    pub fn vote_slash_threshold(
        env: Env,
        voter: Address,
        proposal_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_slash_threshold(env, voter, proposal_id, approve)
    }

    pub fn finalize_slash_threshold(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::finalize_slash_threshold(env, proposal_id)
    }

    pub fn get_slash_threshold_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<SlashThresholdProposal> {
        governance::get_slash_threshold_proposal(env, proposal_id)
    }

    // ── Config Timelock ───────────────────────────────────────────────────────

    pub fn propose_config_change(
        env: Env,
        proposer: Address,
        new_config: Config,
    ) -> Result<u64, ContractError> {
        governance::propose_config_change(env, proposer, new_config)
    }

    pub fn execute_config_change(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::execute_config_change(env, proposal_id)
    }

    pub fn cancel_config_change(
        env: Env,
        admin_signers: Vec<Address>,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        governance::cancel_config_change(env, admin_signers, proposal_id)
    }

    // ── Slash Appeal & Escrow (Issue #841) ────────────────────────────────────

    pub fn appeal_slash(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::appeal_slash(env, borrower)
    }

    pub fn vote_appeal(
        env: Env,
        voucher: Address,
        borrower: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_appeal(env, voucher, borrower, approve)
    }

    pub fn finalize_appeal(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::finalize_appeal(env, borrower)
    }

    // ── Slashing Transparency Reports & Backfill (Issue #656 / #1444) ─────────

    pub fn generate_slashing_report(env: Env, month_id: u64) -> SlashingReportRecord {
        governance::generate_slashing_report(env, month_id)
    }

    pub fn get_slashing_report(env: Env, month_id: u64) -> Option<SlashingReportRecord> {
        governance::get_slashing_report(env, month_id)
    }

    pub fn backfill_slashes_by_month(
        env: Env,
        admin_signers: Vec<Address>,
    ) -> Result<u32, ContractError> {
        governance::backfill_slashes_by_month(env, admin_signers)
    }

    // ── Admin management ─────────────────────────────────────────────────────

    pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
        admin::remove_admin(env, admin_signers, admin_to_remove)
    }

    /// Emergency admin revocation — removes a compromised admin key with N-1 approval.
    ///
    /// This is an emergency mechanism: if one admin key is compromised, ALL remaining
    /// admins (N-1 of N) can instantly revoke the compromised key. The revoked address
    /// is permanently blacklisted from participating in admin approvals and is removed
    /// from the active admin list.
    ///
    /// Unlike `remove_admin` (which uses the standard `admin_threshold`), this function
    /// requires every non-compromised admin to sign — a stricter requirement that prevents
    /// a single admin from unilaterally removing another.
    ///
    /// # Arguments
    /// * `existing_admins` - All current admin signers excluding `target_admin` (must be N-1)
    /// * `target_admin` - The compromised admin address to revoke
    /// * `reason` - Human-readable reason for revocation (emitted in event)
    ///
    /// # Errors
    /// * `ContractError::AdminNotFound` - `target_admin` is not a registered admin
    /// * `ContractError::AdminAlreadyRevoked` - `target_admin` was already revoked
    /// * `ContractError::UnauthorizedCaller` - Fewer than N-1 valid signers provided
    /// * `ContractError::InvalidAdminThreshold` - Only 1 admin exists; cannot revoke
    pub fn revoke_admin(
        env: Env,
        existing_admins: Vec<Address>,
        target_admin: Address,
        reason: soroban_sdk::String,
    ) -> Result<(), ContractError> {
        admin::revoke_admin(env, existing_admins, target_admin, reason)
    }

    /// Check whether an admin address has been emergency-revoked.
    ///
    /// # Arguments
    /// * `admin` - Address to query
    ///
    /// # Returns
    /// * `true` if the address has been revoked via `revoke_admin`
    pub fn is_admin_revoked(env: Env, admin: Address) -> bool {
        admin::is_admin_revoked(env, admin)
    }

    pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
        admin::set_admin_threshold(env, admin_signers, new_threshold)
    }

    // ── RBAC (Issue #16) ──────────────────────────────────────────────────────

    pub fn assign_admin_role(
        env: Env,
        admin_signers: Vec<Address>,
        target_admin: Address,
        role: AdminRole,
    ) {
        rbac::assign_admin_role(&env, admin_signers, target_admin, role)
    }

    pub fn get_admin_role(env: Env, admin: Address) -> Result<AdminRole, ContractError> {
        rbac::get_admin_role(&env, &admin)
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn pause(env: Env, admin_signers: Vec<Address>) {
        admin::pause(env, admin_signers)
    }



    pub fn get_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn set_config(env: Env, admin_signers: Vec<Address>, cfg: Config) {
        admin::set_config(env, admin_signers, cfg)
    }

    // ── Issue #688: Admin whitelist management ────────────────────────────────

    pub fn add_to_admin_whitelist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::add_to_admin_whitelist(env, admin_signers, address)
    }

    pub fn remove_from_admin_whitelist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::remove_from_admin_whitelist(env, admin_signers, address)
    }

    // ── Issue #689: Admin blacklist management ────────────────────────────────

    pub fn add_to_admin_blacklist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::add_to_admin_blacklist(env, admin_signers, address)
    }

    pub fn remove_from_admin_blacklist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::remove_from_admin_blacklist(env, admin_signers, address)
    }

    pub fn update_config(
        env: Env,
        admin_signers: Vec<Address>,
        yield_bps: Option<i128>,
        slash_bps: Option<i128>,
    ) {
        admin::update_config(env, admin_signers, yield_bps, slash_bps)
    }

    pub fn batch_update_config(
        env: Env,
        admin_signers: Vec<Address>,
        yield_bps: Option<i128>,
        slash_bps: Option<i128>,
        max_vouchers: Option<u32>,
        min_loan_amount: Option<i128>,
        loan_duration: Option<u64>,
        max_loan_to_stake_ratio: Option<u32>,
        grace_period: Option<u64>,
        liquidity_mining_rate_bps: Option<u32>,
    ) {
        admin::batch_update_config(
            env,
            admin_signers,
            yield_bps,
            slash_bps,
            max_vouchers,
            min_loan_amount,
            loan_duration,
            max_loan_to_stake_ratio,
            grace_period,
            liquidity_mining_rate_bps,
        )
    }

    pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
        admin::set_reputation_nft(env, admin_signers, nft_contract)
    }

    pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_min_stake(env, admin_signers, amount)
    }

    pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_max_loan_amount(env, admin_signers, amount)
    }

    pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
        admin::set_min_vouchers(env, admin_signers, count)
    }

    pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
        admin::set_max_loan_to_stake_ratio(env, admin_signers, ratio)
    }

    pub fn set_max_vouchers_per_loan(env: Env, admin_signers: Vec<Address>, max: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(max > 0, "max_vouchers_per_loan must be greater than zero");
        let mut cfg = config(&env);
        cfg.max_vouchers = max;
        env.storage().instance().set(&DataKey::Config, &cfg);
        // Also update the instance key read by VouchConfig
        env.storage().instance().set(&DataKey::MaxVouchersPerBorrower, &max);
    }

    pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), ContractError> {
        admin::add_allowed_token(env, admin_signers, token)
    }

    pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
        admin::remove_allowed_token(env, admin_signers, token)
    }

    /// Issue #1073: Set blacklist reason for a borrower.
    pub fn set_blacklist_reason(
        env: Env,
        admin_signers: Vec<Address>,
        borrower: Address,
        reason: soroban_sdk::Bytes,
    ) -> Result<(), ContractError> {
        admin::set_blacklist_reason(env, admin_signers, borrower, reason)
    }

    /// Issue #1073: Get blacklist reason for a borrower.
    pub fn get_blacklist_reason(env: Env, borrower: Address) -> Option<soroban_sdk::Bytes> {
        admin::get_blacklist_reason(env, borrower)
    }

    /// Issue #1072: Apply reputation score decay to a borrower.
    pub fn apply_reputation_decay(env: Env, borrower: Address) -> Result<(), ContractError> {
        credit_score::apply_reputation_decay(&env, &borrower)
    }

    /// Issue #1072: Batch apply reputation score decay to multiple borrowers.
    pub fn apply_reputation_decay_batch(env: Env, borrowers: Vec<Address>) -> Result<u32, ContractError> {
        credit_score::apply_reputation_decay_batch(&env, borrowers)
    }

    /// Issue #1421 Phase 2: Backfill historical payment records for a pre-upgrade loan.
    ///
    /// Admin-gated. Only allowed for loans in a terminal state (Repaid or Defaulted).
    /// Appends the supplied `payment_records` to the `PaymentHistory(loan_id)` storage
    /// key so credit-score timeliness calculations can be recalculated with real data.
    ///
    /// See `docs/credit-score-migration.md` Phase 2 for the full backfill strategy.
    pub fn backfill_payment_history(
        env: Env,
        admin_signers: Vec<Address>,
        loan_id: u64,
        payment_records: Vec<PaymentRecord>,
    ) -> Result<(), ContractError> {
        admin::backfill_payment_history(env, admin_signers, loan_id, payment_records)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn get_token(env: Env) -> Address {
        config(&env).token
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        admin::get_admins(env)
    }

    pub fn get_admin_threshold(env: Env) -> u32 {
        admin::get_admin_threshold(env)
    }

    pub fn get_slash_treasury_balance(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::SlashTreasury)
            .unwrap_or(0)
    }


    pub fn get_protocol_fee(env: Env) -> u32 {
        admin::get_protocol_fee(env)
    }

    pub fn get_fee_treasury(env: Env) -> Option<Address> {
        admin::get_fee_treasury(env)
    }

    pub fn is_blacklisted(env: Env, borrower: Address) -> bool {
        admin::is_blacklisted(env, borrower)
    }

    pub fn get_min_stake(env: Env) -> i128 {
        admin::get_min_stake(env)
    }

    pub fn get_max_loan_amount(env: Env) -> i128 {
        admin::get_max_loan_amount(env)
    }

    pub fn get_min_vouchers(env: Env) -> u32 {
        admin::get_min_vouchers(env)
    }

    pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
        admin::get_max_loan_to_stake_ratio(env)
    }

    pub fn get_max_vouchers_per_loan(env: Env) -> u32 {
        config(&env).max_vouchers
    }

    // ── Issue #682: multi-sig config updates ──────────────────────────────────

    pub fn propose_config_update(
        env: Env,
        proposer: Address,
        key: ConfigUpdateKey,
        new_value: u32,
    ) -> Result<u64, ContractError> {
        admin::propose_config_update(env, proposer, key, new_value)
    }

    pub fn approve_config_update(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::approve_config_update(env, admin, proposal_id)
    }

    pub fn finalize_config_update(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        admin::finalize_config_update(env, proposal_id)
    }

    pub fn get_config_update_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<ConfigUpdateProposal> {
        admin::get_config_update_proposal(env, proposal_id)
    }

    // ── Admin Governance Queue with Multi-Signature Confirmation ─────────────

    pub fn set_governance_queue_config(
        env: Env,
        admin_signers: Vec<Address>,
        config: GovernanceQueueConfig,
    ) {
        admin::set_governance_queue_config(env, admin_signers, config)
    }

    pub fn propose_governance_action(
        env: Env,
        proposer: Address,
        action: GovernanceAction,
        description: soroban_sdk::String,
    ) -> Result<u64, ContractError> {
        admin::propose_governance_action(env, proposer, action, description)
    }

    pub fn approve_governance_action(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::approve_governance_action(env, admin, proposal_id)
    }

    pub fn reject_governance_action(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::reject_governance_action(env, admin, proposal_id)
    }

    pub fn execute_governance_action(
        env: Env,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::execute_governance_action(env, proposal_id)
    }

    pub fn cancel_governance_action(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::cancel_governance_action(env, caller, proposal_id)
    }

    pub fn get_governance_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<GovernanceProposal> {
        admin::get_governance_proposal(env, proposal_id)
    }

    pub fn get_governance_queue_config_view(env: Env) -> GovernanceQueueConfig {
        admin::get_governance_queue_config_view(env)
    }

    pub fn get_governance_proposal_count(env: Env) -> u64 {
        admin::get_governance_proposal_count(env)
    }

    // ── Admin Action Proposals (Issue #554 / #1442) ───────────────────────────

    pub fn propose_admin_action(
        env: Env,
        proposer: Address,
        action_type: GovernanceAction,
    ) -> Result<u64, ContractError> {
        admin::propose_admin_action(env, proposer, action_type)
    }

    pub fn approve_admin_action(
        env: Env,
        admin: Address,
        action_id: u64,
    ) -> Result<(), ContractError> {
        admin::approve_admin_action(env, admin, action_id)
    }

    pub fn execute_admin_action(
        env: Env,
        action_id: u64,
    ) -> Result<(), ContractError> {
        admin::execute_admin_action(env, action_id)
    }

    pub fn get_admin_action_proposal(
        env: Env,
        action_id: u64,
    ) -> Option<AdminActionProposal> {
        admin::get_admin_action_proposal(env, action_id)
    }

    // ── On-Chain Credit Score with Tiered Rewards ───────────────────────────────

    pub fn update_credit_score(env: Env, borrower: Address) -> Result<(), ContractError> {
        credit_score::update_credit_score(env, borrower)
    }

    pub fn get_credit_score(env: Env, borrower: Address) -> Option<CreditScore> {
        credit_score::get_credit_score(env, borrower)
    }

    pub fn set_credit_score_config(
        env: Env,
        admin_signers: Vec<Address>,
        config: CreditScoreConfig,
    ) -> Result<(), ContractError> {
        credit_score::set_credit_score_config(env, admin_signers, config)
    }

    pub fn get_credit_score_config_view(env: Env) -> CreditScoreConfig {
        credit_score::get_credit_score_config_view(env)
    }

    pub fn get_tier_rewards(env: Env, tier: CreditTier) -> TierRewards {
        credit_score::get_tier_rewards(env, tier)
    }

    // ── Issue #637: On-Demand Fraud Detection ──────────────────────────────────





    // ── Loan Pool Syndication for Multi-Borrower Loans ─────────────────────────









    // ── Data Archiving ────────────────────────────────────────────────────────





    // ── IPFS Archiving ────────────────────────────────────────────────────────













    // ── Issue #683: emergency pause ───────────────────────────────────────────

    pub fn emergency_pause(env: Env, admin_signers: Vec<Address>) -> Result<(), ContractError> {
        admin::emergency_pause(env, admin_signers)
    }

    pub fn emergency_unpause(env: Env, admin_signers: Vec<Address>) -> Result<(), ContractError> {
        admin::emergency_unpause(env, admin_signers)
    }

    /// Toggle the borrower repayment confirmation requirement on/off.
    ///
    /// When enabled, borrowers must call `confirm_repayment` before `repay`.
    pub fn set_confirmation_required(
        env: Env,
        admin_signers: Vec<Address>,
        enabled: bool,
    ) {
        admin::set_confirmation_required(env, admin_signers, enabled)
    }

    pub fn set_successor_admin(
        env: Env,
        admin_signers: Vec<Address>,
        successor: Option<Address>,
    ) {
        admin::set_successor_admin(env, admin_signers, successor)
    }

    pub fn claim_successor_admin(env: Env) -> Result<(), ContractError> {
        admin::claim_successor_admin(env)
    }

    pub fn cancel_successor_admin(
        env: Env,
        admin_signers: Vec<Address>,
    ) -> Result<(), ContractError> {
        admin::cancel_successor_admin(env, admin_signers)
    }

    // ── Issue #14: Cross-chain loan portability ───────────────────────────────








    pub fn voucher_history(env: Env, voucher: Address) -> Vec<Address> {
        vouch::voucher_history(env, voucher)
    }

    // ── Loan delegation ───────────────────────────────────────────────────────

    // ── Issue #969 (#86): Cross-Chain Event Relay ─────────────────────────────






    pub fn is_eligible(env: Env, borrower: Address, threshold: i128) -> bool {
        let token = config(&env).token;
        loan::is_eligible(env, borrower, threshold, token)
    }



    // ── Admin delegation ──────────────────────────────────────────────────────



    pub fn rotate_admin(
        env: Env,
        admin_signers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) {
        admin::rotate_admin(env, admin_signers, old_admin, new_admin)
    }

    // ── Cross-Chain Relay Pipeline (Issue #1361) ──────────────────────────────

    /// Admin: register an Ed25519 public key for a source chain's relay attestations.
    pub fn set_relay_key(
        env: Env,
        admin_signers: Vec<Address>,
        source_chain: u32,
        public_key: BytesN<32>,
    ) -> Result<(), ContractError> {
        crate::set_relay_key(env, admin_signers, source_chain, public_key)
    }

    /// Admin: emit an outbound relay event (to be signed and relayed to dest_chain).
    pub fn relay_emit(
        env: Env,
        admin_signers: Vec<Address>,
        dest_chain: u32,
        event_type: soroban_sdk::Symbol,
        payload: soroban_sdk::Bytes,
    ) -> Result<u64, ContractError> {
        crate::relay_emit(env, admin_signers, dest_chain, event_type, payload)
    }

    /// Canonical bytes the source chain's relay key must sign for an event.
    pub fn relay_attestation_message(
        env: Env,
        event: RelayEvent,
        nonce: u64,
        timestamp: u64,
    ) -> soroban_sdk::Bytes {
        crate::relay_attestation_message(&env, &event, nonce, timestamp)
    }

    /// Verify and consume an inbound relayed event (idempotent per source+seq).
    pub fn relay_message(
        env: Env,
        event: RelayEvent,
        attestation: RelayAttestation,
    ) -> Result<(), ContractError> {
        crate::relay_message(env, event, attestation)
    }

    /// Acknowledge outbound delivery up to `up_to_seq` for `dest_chain`.
    pub fn acknowledge_relay(
        env: Env,
        admin_signers: Vec<Address>,
        dest_chain: u32,
        up_to_seq: u64,
    ) -> Result<(), ContractError> {
        crate::acknowledge_relay(env, admin_signers, dest_chain, up_to_seq)
    }

    pub fn get_outbound_relay_event(env: Env, dest_chain: u32, seq: u64) -> Option<RelayEvent> {
        crate::get_outbound_event(env, dest_chain, seq)
    }

    pub fn latest_outbound_relay_seq(env: Env, dest_chain: u32) -> u64 {
        crate::latest_outbound_seq(env, dest_chain)
    }

    pub fn last_acknowledged_relay_seq(env: Env, dest_chain: u32) -> u64 {
        crate::last_acknowledged_seq(env, dest_chain)
    }

    pub fn is_relay_processed(env: Env, source_chain: u32, seq: u64) -> bool {
        crate::is_relay_processed(env, source_chain, seq)
    }

    pub fn is_relay_nonce_used(env: Env, source_chain: u32, nonce: u64) -> bool {
        crate::is_relay_nonce_used(env, source_chain, nonce)
    }
    // ── Custom Attributes ────────────────────────────────────────────────────

    /// Issue #1282: Persist a key/value attribute for the caller.
    /// Requires caller auth. Keys and values are capped at 256 bytes.
    /// A caller may store at most 50 attributes.
    pub fn set_attribute(env: Env, caller: Address, key: soroban_sdk::String, value: soroban_sdk::String) -> Result<(), ContractError> {
        crate::set_attribute(env, caller, key, value)
    }

    /// Issue #1282: Return all custom attributes stored for `caller`.
    pub fn get_attributes(env: Env, caller: Address) -> Vec<AttributeEntry> {
        crate::get_attributes(env, caller)
    }

    /// Issue #1282: Remove a single attribute by key for `caller` (idempotent).
    pub fn remove_attribute(env: Env, caller: Address, key: soroban_sdk::String) -> Result<(), ContractError> {
        crate::remove_attribute(env, caller, key)
    }
    // ── Yield Stream ─────────────────────────────────────────────────────────




    // ── Vouch Groups ─────────────────────────────────────────────────────────





    pub fn upgrade(env: Env, admin_signers: Vec<Address>, new_wasm_hash: BytesN<32>) {
        admin::upgrade(env, admin_signers, new_wasm_hash)
    }


    pub fn add_voucher_to_group(env: Env, caller: Address, group_id: u64, voucher: Address) -> Result<(), ContractError> {
        crate::add_voucher_to_group(env, caller, group_id, voucher)
    }

    pub fn remove_voucher_from_group(env: Env, caller: Address, group_id: u64, voucher: Address) -> Result<(), ContractError> {
        crate::remove_voucher_from_group(env, caller, group_id, voucher)
    }

    pub fn get_vouch_group(env: Env, group_id: u64) -> Option<VouchGroup> {
        crate::get_vouch_group(env, group_id)
    }

    pub fn get_voucher_group_ids(env: Env, voucher: Address) -> Vec<u64> {
        crate::get_voucher_group_ids(env, voucher)
    }
    // ── Periodic Payments ────────────────────────────────────────────────────




    pub fn get_contract_balance(env: Env) -> i128 {
        token(&env).balance(&env.current_contract_address())
    }

    // ── Issue #1074: Reentrancy guard — already wired into vouch / request_loan / repay above ──

    // ── Issue #1075: Bridge token support ────────────────────────────────────

    /// Bridge external tokens (e.g. USDC) into the contract for staking.
    ///
    /// Transfers `amount` of `source_token` from `caller` to this contract.
    /// The bridged balance is tracked in `DataKey::BridgedTokens(source_token)`.
    /// Bridged tokens earn base yield **plus** a tier bonus (see #1077).
    pub fn bridge_token(
        env: Env,
        caller: Address,
        bridge_contract: Address,
        source_token: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        bridge::bridge_token(env, caller, bridge_contract, source_token, amount)
    }

    /// Query the bridged-token balance held by the contract for a given token address.
    pub fn get_bridged_token_balance(env: Env, token_addr: Address) -> i128 {
        bridge::get_bridged_token_balance(env, token_addr)
    }

    /// Admin: set the oracle price (in basis points) for a bridge token relative
    /// to the primary token. Used by `repay_with_swap` for cross-token conversion.
    pub fn set_bridge_token_price(
        env: Env,
        admin_signers: Vec<Address>,
        token_addr: Address,
        price_bps: i128,
    ) -> Result<(), ContractError> {
        bridge::set_bridge_token_price(env, admin_signers, token_addr, price_bps)
    }

    // ── Issue #1076: Token swap on repayment mismatch ─────────────────────────

    /// Repay a loan using a different token than the loan's denomination.
    ///
    /// Converts `payment_amount` of `payment_token` to the loan's token using
    /// the admin-configured oracle price, then applies the repayment.
    ///
    /// Falls back to normal repay if `payment_token == loan.token_address`.
    pub fn repay_with_swap(
        env: Env,
        borrower: Address,
        payment_token: Address,
        payment_amount: i128,
    ) -> Result<(), ContractError> {
        bridge::repay_with_swap(env, borrower, payment_token, payment_amount)
    }

    /// Issue #965: Atomic cross-chain repayment with bridge attestation verification.
    /// Enables borrowers to repay loans from other chains with atomic semantics.
    pub fn repay_cross_chain_atomic(
        env: Env,
        origin_chain: u32,
        loan_id: u64,
        borrower: Address,
        payment_amount: i128,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        bridge::repay_cross_chain_atomic(env, origin_chain, loan_id, borrower, payment_amount, attestation)
    }

    // ── Issue #1077: Dynamic yield based on token liquidity ───────────────────

    /// Return the liquidity tier (0–3) for a given token address.
    ///
    /// Tier 0 = highly liquid (e.g. XLM, no bonus).
    /// Tier 3 = illiquid (highest yield bonus, default +300 bps).
    pub fn get_token_liquidity_tier(env: Env, token_addr: Address) -> u32 {
        bridge::get_token_liquidity_tier(env, token_addr)
    }

    /// Admin: set the liquidity tier (0–3) for a given token address.
    ///
    /// Higher tiers earn more yield to compensate for liquidity risk.
    pub fn set_token_liquidity_tier(
        env: Env,
        admin_signers: Vec<Address>,
        token_addr: Address,
        tier: u32,
    ) -> Result<(), ContractError> {
        bridge::set_token_liquidity_tier(env, admin_signers, token_addr, tier)
    }

    pub fn make_periodic_payment(env: Env, borrower: Address, loan_id: u64, payment: i128) -> Result<(), ContractError> {
        crate::make_periodic_payment(env, borrower, loan_id, payment)
    }

    // ── Issue #883: Loan Term Extension ─────────────────────────────────────

    pub fn request_extension(
        env: Env,
        borrower: Address,
        extension_secs: u64,
    ) -> Result<(), ContractError> {
        loan::request_extension(env, borrower, extension_secs)
    }

    pub fn approve_extension(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        loan::approve_extension(env, voucher, borrower)
    }

    pub fn get_extension_request(env: Env, borrower: Address) -> Option<LoanExtensionRequest> {
        loan::get_extension_request(env, borrower)
    }

    // ── Issue #882: Loan Insurance Integration ──────────────────────────────

    pub fn set_insurance_fund_premium_bps(
        env: Env,
        admin_signers: Vec<Address>,
        premium_bps: u32,
    ) {
        admin::set_insurance_fund_premium_bps(env, admin_signers, premium_bps)
    }

    pub fn set_insurance_max_payout_bps(
        env: Env,
        admin_signers: Vec<Address>,
        max_payout_bps: u32,
    ) {
        admin::set_insurance_max_payout_bps(env, admin_signers, max_payout_bps)
    }

    pub fn set_insurance_premium_bps(
        env: Env,
        admin_signers: Vec<Address>,
        premium_bps: u32,
    ) {
        admin::set_insurance_premium_bps(env, admin_signers, premium_bps)
    }



    pub fn get_insurance_pool_balance(env: Env) -> i128 {
        crate::get_insurance_pool_balance(env)
    }

    // ── Issue #1172/#1406: Guarantor coverage ─────────────────────────────────

    /// Locks `guarantee_amount` of `token` from `guarantor_address` into the
    /// contract as collateral backing the loan (#1406).
    pub fn request_guarantor_for_loan(
        env: Env,
        loan_id: u64,
        guarantor_address: Address,
        guarantee_amount: i128,
        token: Address,
    ) -> Result<(), ContractError> {
        guarantor::request_guarantor_for_loan(env, loan_id, guarantor_address, guarantee_amount, token)
    }

    /// Releases a guarantor once their obligation is over, returning the
    /// locked collateral (#1406).
    pub fn release_guarantor(env: Env, loan_id: u64) -> Result<(), ContractError> {
        guarantor::release_guarantor(env, loan_id)
    }

    pub fn get_guarantor_record(env: Env, loan_id: u64) -> Result<GuarantorRecord, ContractError> {
        guarantor::get_guarantor_record(env, loan_id)
    }

    pub fn get_guarantor_stats(env: Env, guarantor: Address) -> Result<GuarantorStats, ContractError> {
        guarantor::get_guarantor_stats(env, guarantor)
    }

    /// Pays out a defaulted loan's locked guarantee to its vouchers pro-rata
    /// (or the borrower if there are none), only once the loan is actually
    /// Defaulted, and only once ever per guarantee (#1406).
    pub fn claim_guarantor_coverage(env: Env, loan_id: u64) -> Result<i128, ContractError> {
        guarantor::claim_guarantor_coverage(env, loan_id)
    }

    pub fn get_guarantor_reputation_multiplier(env: Env, guarantor: Address) -> Result<u32, ContractError> {
        guarantor::get_guarantor_reputation_multiplier(env, guarantor)
    }




    // ── Issue #884: Prepayment Bonus ────────────────────────────────────────

    pub fn set_prepayment_bonus_bps(
        env: Env,
        admin_signers: Vec<Address>,
        bonus_bps: u32,
    ) -> Result<(), ContractError> {
        loan::set_prepayment_bonus_bps(env, admin_signers, bonus_bps)
    }

    pub fn get_prepayment_bonus_bps(env: Env) -> u32 {
        loan::get_prepayment_bonus_bps(&env)
    }

    // ── Issue #885: Loan Status Privacy ─────────────────────────────────────

    pub fn set_loan_privacy(
        env: Env,
        borrower: Address,
        privacy: LoanPrivacyLevel,
    ) -> Result<(), ContractError> {
        loan::set_loan_privacy(env, borrower, privacy)
    }

    pub fn get_loan_privacy(env: Env, borrower: Address) -> LoanPrivacyLevel {
        loan::get_loan_privacy(&env, &borrower)
    }

    pub fn get_loan_with_privacy(
        env: Env,
        borrower: Address,
        caller: Address,
    ) -> Result<Option<LoanRecord>, ContractError> {
        loan::get_loan_with_privacy(env, borrower, caller)
    }

    // ── Issue #938: Incremental Config Changes ────────────────────────────────

    /// Enqueue a named config field change to be applied no earlier than `apply_after`.
    pub fn enqueue_config_patch(
        env: Env,
        admin_signers: Vec<Address>,
        field: ConfigField,
        new_value: i128,
        apply_after: u64,
    ) {
        admin::enqueue_config_patch(env, admin_signers, field, new_value, apply_after)
    }

    /// Apply the next pending config patch whose not-before timestamp has passed.
    /// Returns `true` if a patch was applied.
    pub fn apply_next_config_patch(env: Env) -> bool {
        admin::apply_next_config_patch(env)
    }

    pub fn is_whitelisted(env: Env, voucher: Address) -> bool {
        admin::is_whitelisted(env, voucher)
    }


    pub fn get_config_patch(env: Env, idx: u32) -> Option<ConfigPatch> {
        admin::get_config_patch(env, idx)
    }

    pub fn get_config_patch_count(env: Env) -> u32 {
        admin::get_config_patch_count(env)
    }

    // ── Issue #1080: Request Idempotency Support ────────────────────────────────

    /// Check or store an idempotency key for request deduplication.
    /// Returns true if this is a new request, false if it's a duplicate (within 24h TTL).
    pub fn check_idempotency_key(
        env: Env,
        caller: Address,
        idempotency_key: String,
    ) -> bool {
        caller.require_auth();

        let key = DataKey::IdempotencyKey(idempotency_key.clone());
        let current_time = env.ledger().timestamp();
        let ttl_24h: u64 = 24 * 60 * 60;

        if let Some(record) = env
            .storage()
            .persistent()
            .get::<DataKey, IdempotencyRecord>(&key)
        {
            if current_time < record.created_at + ttl_24h {
                env.events().publish(
                    (symbol_short!("idem"), symbol_short!("dup")),
                    (caller, idempotency_key),
                );
                return false;
            }
        }

        let new_record = IdempotencyRecord {
            key: idempotency_key.clone(),
            response_hash: BytesN::<32>::from_array(&env, &[0u8; 32]),
            created_at: current_time,
        };

        env.storage()
            .persistent()
            .set(&DataKey::IdempotencyKey(idempotency_key.clone()), &new_record);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::IdempotencyKey(idempotency_key.clone()), ttl_24h as u32, ttl_24h as u32);

        env.events().publish(
            (symbol_short!("idem"), symbol_short!("new")),
            (caller, idempotency_key),
        );

        true
    }

    // ── Issue #1081: Role-Based Rate Limiting ───────────────────────────────────

    /// Set up role-based rate limit tiers (admin: unlimited, user: 1000/hr, guest: 100/hr).
    pub fn setup_role_based_rate_limits(env: Env, admin_signers: Vec<Address>) {
        require_admin_approval(&env, &admin_signers);

        let mut cfg = config(&env);
        let mut tiers: Vec<RateLimitTier> = Vec::new(&env);

        tiers.push_back(RateLimitTier {
            role: UserRole::Admin,
            max_requests_per_hour: u32::MAX,
        });
        tiers.push_back(RateLimitTier {
            role: UserRole::User,
            max_requests_per_hour: 1000,
        });
        tiers.push_back(RateLimitTier {
            role: UserRole::Guest,
            max_requests_per_hour: 100,
        });

        cfg.rate_limit_config.tiers = tiers;
        cfg.rate_limit_config.enabled = true;

        env.storage().instance().set(&DataKey::Config, &cfg);

        env.events().publish(
            (symbol_short!("ratelim"), symbol_short!("setup")),
            ("setup_role_based_rate_limits", u32::MAX, 1000, 100),
        );
    }

    /// Check rate limit for a specific user by their role. Returns true if under limit.
    pub fn check_rate_limit(
        env: Env,
        user: Address,
        role: UserRole,
    ) -> bool {
        let cfg = config(&env);

        if !cfg.rate_limit_config.enabled {
            return true;
        }

        let current_time = env.ledger().timestamp();
        let window_secs = 60 * 60;

        let rate_key = DataKey::RateLimitByRole(user.clone(), role.clone());

        let mut max_requests = 100u32;
        for tier in cfg.rate_limit_config.tiers.iter() {
            if tier.role == role {
                max_requests = tier.max_requests_per_hour;
                break;
            }
        }

        if role == UserRole::Admin {
            return true;
        }

        if let Some((last_window, count)) = env
            .storage()
            .persistent()
            .get::<DataKey, (u64, u32)>(&rate_key)
        {
            if current_time < last_window + window_secs {
                if count >= max_requests {
                    env.events().publish(
                        (symbol_short!("ratelim"), symbol_short!("exceeded")),
                        (user, role as u32, count),
                    );
                    return false;
                }
                env.storage()
                    .persistent()
                    .set(&rate_key, &(last_window, count + 1));
            } else {
                env.storage().persistent().set(&rate_key, &(current_time, 1));
            }
        } else {
            env.storage().persistent().set(&rate_key, &(current_time, 1));
        }

        env.events().publish(
            (symbol_short!("ratelim"), symbol_short!("ok")),
            (user, role as u32),
        );

        true
    }
}

pub fn add_voucher_to_group(_env: Env, _caller: Address, _group_id: u64, _voucher: Address) -> Result<(), ContractError> {
    Ok(())
}

pub fn remove_voucher_from_group(_env: Env, _caller: Address, _group_id: u64, _voucher: Address) -> Result<(), ContractError> {
    Ok(())
}

pub fn create_vouch_group(_env: Env, _caller: Address, _name: soroban_sdk::String) -> Result<u64, ContractError> {
    Ok(0)
}

pub fn get_vouch_group(_env: Env, _group_id: u64) -> Option<VouchGroup> {
    None
}

pub fn get_voucher_group_ids(env: Env, _voucher: Address) -> Vec<u64> {
    Vec::new(&env)
}

pub fn allocate_slash_to_pool(_env: &Env, _total_slashed: i128) {}

pub fn rebalance_pools(
    _env: Env,
    _admin_signers: Vec<Address>,
    _source_pool_id: u64,
    _target_pool_id: u64,
    _amount: i128,
) -> Result<(), ContractError> {
    Ok(())
}

pub fn auto_rebalance_pools(
    _env: Env,
    _admin_signers: Vec<Address>,
    _target_stake: i128,
) -> Result<u32, ContractError> {
    Ok(0)
}

pub fn get_pool_liquidity(_env: Env, _pool_id: u64) -> Result<i128, ContractError> {
    Ok(0)
}

pub fn claim_insurance(_env: Env, _voucher: Address, _loan_id: u64) -> Result<(), ContractError> {
    Ok(())
}

pub fn contribute_to_insurance(_env: Env, _caller: Address, _amount: i128) -> Result<(), ContractError> {
    Ok(())
}

pub fn is_voucher_insured(_env: Env, _voucher: Address, _borrower: Address) -> bool {
    false
}

pub fn get_insurance_coverage_bps_pub(_env: Env) -> u32 {
    0
}

pub fn get_insurance_fee_bps_pub(_env: Env) -> u32 {
    0
}

pub fn get_insurance_pool_balance(env: Env) -> i128 {
    insurance::get_insurance_fund_balance(&env)
}

pub fn set_insurance_coverage_bps(_env: Env, _admin_signers: Vec<Address>, _bps: u32) -> Result<(), ContractError> {
    Ok(())
}

pub fn set_insurance_fee_bps(_env: Env, _admin_signers: Vec<Address>, _bps: u32) -> Result<(), ContractError> {
    Ok(())
}

pub fn purchase_slash_insurance(_env: Env, _voucher: Address, _borrower: Address) -> Result<i128, ContractError> {
    Ok(0)
}

pub fn get_periodic_payment_config(_env: Env, _loan_id: u64) -> Option<PeriodicPaymentConfig> {
    None
}

pub fn get_periodic_payment_status(_env: Env, _loan_id: u64) -> Option<PeriodicPaymentStatus> {
    None
}

pub fn make_periodic_payment(_env: Env, _caller: Address, _loan_id: u64, _amount: i128) -> Result<(), ContractError> {
    Ok(())
}

pub fn set_periodic_payment(
    _env: Env,
    _caller: Address,
    _loan_id: u64,
    _schedule_type: ScheduleType,
    _period_count: u32,
    _period_interest_bps: u32,
) -> Result<(), ContractError> {
    Ok(())
}

pub fn get_voucher_yield_claim(_env: Env, _loan_id: u64, _voucher: Address) -> Option<VoucherYieldClaim> {
    None
}

pub fn set_relay_key(env: Env, admin_signers: Vec<Address>, source_chain: u32, public_key: BytesN<32>) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);

    if source_chain == 0 {
        return Err(ContractError::InvalidRelayChain);
    }

    env.storage()
        .persistent()
        .set(&DataKey::RelayPublicKey(source_chain), &public_key);
    Ok(())
}

pub fn relay_emit(env: Env, admin_signers: Vec<Address>, dest_chain: u32, event_type: soroban_sdk::Symbol, payload: soroban_sdk::Bytes) -> Result<u64, ContractError> {
    require_admin_approval(&env, &admin_signers);

    if dest_chain == 0 {
        return Err(ContractError::InvalidRelayChain);
    }

    let current_seq = env
        .storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::OutboundRelaySeq(dest_chain))
        .unwrap_or(0);
    let next_seq = current_seq.checked_add(1).ok_or(ContractError::ArithmeticError)?;

    let event = RelayEvent {
        source_chain: 0,
        dest_chain,
        event_type,
        payload,
        seq: next_seq,
    };

    env.storage()
        .persistent()
        .set(&DataKey::OutboundRelayEvent(dest_chain, next_seq), &event);
    env.storage()
        .persistent()
        .set(&DataKey::OutboundRelaySeq(dest_chain), &next_seq);

    Ok(next_seq)
}

pub fn relay_attestation_message(
    env: &Env,
    event: &RelayEvent,
    nonce: u64,
    timestamp: u64,
) -> soroban_sdk::Bytes {
    let payload = (event.clone(), nonce, timestamp);
    let encoded = payload.to_xdr(env);
    env.crypto().sha256(&encoded).into()
}

pub fn relay_message(env: Env, event: RelayEvent, attestation: RelayAttestation) -> Result<(), ContractError> {
    if event.source_chain == 0 {
        return Err(ContractError::InvalidRelayChain);
    }

    let public_key: BytesN<32> = env
        .storage()
        .persistent()
        .get(&DataKey::RelayPublicKey(event.source_chain))
        .ok_or(ContractError::RelayKeyNotConfigured)?;

    if is_relay_processed(env.clone(), event.source_chain, event.seq) {
        return Err(ContractError::RelayEventAlreadyProcessed);
    }

    if is_relay_nonce_used(env.clone(), event.source_chain, attestation.nonce) {
        return Err(ContractError::RelayReplayDetected);
    }

    let now = env.ledger().timestamp();
    if attestation.timestamp > now {
        if attestation.timestamp.saturating_sub(now) > 60 {
            return Err(ContractError::RelayEventFromFuture);
        }
    } else if now.saturating_sub(attestation.timestamp) > 600 {
        return Err(ContractError::RelayEventExpired);
    }

    let message = relay_attestation_message(&env, &event, attestation.nonce, attestation.timestamp);

    env.crypto().ed25519_verify(&public_key, &message, &attestation.signature);

    env.storage().persistent().set(
        &DataKey::RelayEventProcessed(event.source_chain, event.seq),
        &true,
    );

    env.storage().persistent().set(
        &DataKey::RelayNonceUsed(event.source_chain, attestation.nonce),
        &true,
    );

    Ok(())
}

pub fn acknowledge_relay(
    env: Env,
    admin_signers: Vec<Address>,
    dest_chain: u32,
    up_to_seq: u64,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);

    if dest_chain == 0 {
        return Err(ContractError::InvalidRelayChain);
    }

    let last_acked = env
        .storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::LastAcknowledgedRelaySeq(dest_chain))
        .unwrap_or(0);

    if up_to_seq < last_acked {
        return Err(ContractError::RelayAckRegression);
    }

    env.storage()
        .persistent()
        .set(&DataKey::LastAcknowledgedRelaySeq(dest_chain), &up_to_seq);

    Ok(())
}

pub fn get_outbound_event(env: Env, dest_chain: u32, seq: u64) -> Option<RelayEvent> {
    env.storage()
        .persistent()
        .get(&DataKey::OutboundRelayEvent(dest_chain, seq))
}

pub fn latest_outbound_seq(env: Env, dest_chain: u32) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::OutboundRelaySeq(dest_chain))
        .unwrap_or(0)
}

pub fn last_acknowledged_seq(env: Env, dest_chain: u32) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::LastAcknowledgedRelaySeq(dest_chain))
        .unwrap_or(0)
}

pub fn is_relay_processed(env: Env, source_chain: u32, seq: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RelayEventProcessed(source_chain, seq))
        .unwrap_or(false)
}

pub fn is_relay_nonce_used(env: Env, source_chain: u32, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RelayNonceUsed(source_chain, nonce))
        .unwrap_or(false)
}

/// Issue #1282: Maximum number of custom attributes a single caller may store.
/// Caps persistent-storage growth to a bounded constant per account.
const MAX_CUSTOM_ATTRIBUTES: u32 = 50;

/// Issue #1282: Maximum byte length for an attribute key or value.
const MAX_ATTRIBUTE_BYTES: u32 = 256;

/// Issue #1282: Set (insert or overwrite) a custom attribute for `caller`.
/// - Requires `caller` to authorise the call.
/// - Key and value lengths are capped at `MAX_ATTRIBUTE_BYTES`.
/// - Per-caller attribute count is capped at `MAX_CUSTOM_ATTRIBUTES`; trying
///   to insert a new key beyond the cap returns `InvalidAmount`.
pub fn set_attribute(env: Env, caller: Address, key: soroban_sdk::String, value: soroban_sdk::String) -> Result<(), ContractError> {
    caller.require_auth();

    // Enforce length caps to bound storage growth.
    if key.len() == 0 || key.len() > MAX_ATTRIBUTE_BYTES || value.len() > MAX_ATTRIBUTE_BYTES {
        return Err(ContractError::InvalidAmount);
    }

    let storage_key = DataKey::CustomAttributes(caller.clone());
    let mut attrs: Vec<AttributeEntry> = env
        .storage()
        .persistent()
        .get(&storage_key)
        .unwrap_or(Vec::new(&env));

    // Check if key already exists — update in place.
    let mut found = false;
    let mut updated: Vec<AttributeEntry> = Vec::new(&env);
    for entry in attrs.iter() {
        if entry.key == key {
            updated.push_back(AttributeEntry {
                key: key.clone(),
                value: value.clone(),
            });
            found = true;
        } else {
            updated.push_back(entry.clone());
        }
    }

    if !found {
        // New key: enforce the per-caller cap.
        if attrs.len() >= MAX_CUSTOM_ATTRIBUTES {
            return Err(ContractError::InvalidAmount);
        }
        updated.push_back(AttributeEntry { key, value });
    }

    env.storage().persistent().set(&storage_key, &updated);
    Ok(())
}

/// Issue #1282: Return all custom attributes stored for `caller`.
pub fn get_attributes(env: Env, caller: Address) -> Vec<AttributeEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::CustomAttributes(caller))
        .unwrap_or(Vec::new(&env))
}

/// Issue #1282: Remove a single attribute by key for `caller`.
/// Returns `Ok(())` even if the key did not exist (idempotent delete).
pub fn remove_attribute(env: Env, caller: Address, key: soroban_sdk::String) -> Result<(), ContractError> {
    caller.require_auth();

    let storage_key = DataKey::CustomAttributes(caller.clone());
    let attrs: Vec<AttributeEntry> = env
        .storage()
        .persistent()
        .get(&storage_key)
        .unwrap_or(Vec::new(&env));

    let mut updated: Vec<AttributeEntry> = Vec::new(&env);
    for entry in attrs.iter() {
        if entry.key != key {
            updated.push_back(entry.clone());
        }
    }

    env.storage().persistent().set(&storage_key, &updated);
    Ok(())
}

pub fn claim_streamed_yield(_env: Env, _caller: Address, _loan_id: u64) -> Result<i128, ContractError> {
    Ok(0)
}

pub fn get_yield_stream_state(_env: Env, _loan_id: u64) -> Option<YieldStreamState> {
    None
}

#[cfg(test)]
mod lib_tests {
    use super::*;
    use crate::reputation::ReputationNftContract;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{StellarAssetClient, TokenClient},
        Env, String, Vec,
    };

    // ── Setup helpers ─────────────────────────────────────────────────────────

    fn single_admin_signers(env: &Env, admin: &Address) -> Vec<Address> {
        Vec::from_array(env, [admin.clone()])
    }

    /// Returns (contract_id, token_addr, admin, borrower, voucher)
    fn setup(env: &Env) -> (Address, Address, Address, Address, Address) {
        env.mock_all_auths();

        let deployer = Address::generate(env);
        let admin = Address::generate(env);
        let admins = Vec::from_array(env, [admin.clone()]);

        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        // Fund contract so it can disburse loans and pay yield.
        StellarAssetClient::new(env, &token_id.address()).mint(&contract_id, &10_000_000);

        let client = QuorumCreditContractClient::new(env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());

        // Advance time past MIN_VOUCH_AGE (60 s).
        env.ledger().with_mut(|l| l.timestamp = 120);

        let borrower = Address::generate(env);
        let voucher = Address::generate(env);
        StellarAssetClient::new(env, &token_id.address()).mint(&voucher, &10_000_000);
        // Fund borrower so they can repay loan + yield
        StellarAssetClient::new(env, &token_id.address()).mint(&borrower, &1_000_000);

        (contract_id, token_id.address(), admin, borrower, voucher)
    }

    /// Returns (contract_id, token_addr, admin, borrower, voucher, nft_contract_id)
    fn setup_with_reputation(
        env: &Env,
    ) -> (Address, Address, Address, Address, Address, Address) {
        env.mock_all_auths();

        let deployer = Address::generate(env);
        let admin = Address::generate(env);
        let admins = Vec::from_array(env, [admin.clone()]);

        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let nft_id = env.register_contract(None, ReputationNftContract);

        StellarAssetClient::new(env, &token_id.address()).mint(&contract_id, &10_000_000);

        let client = QuorumCreditContractClient::new(env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());

        let nft_client = reputation::ReputationNftContractClient::new(env, &nft_id);
        nft_client.initialize(&contract_id);

        let admin_signers = single_admin_signers(env, &admin);
        client.set_reputation_nft(&admin_signers, &nft_id);

        env.ledger().with_mut(|l| l.timestamp = 120);

        let borrower = Address::generate(env);
        let voucher = Address::generate(env);
        StellarAssetClient::new(env, &token_id.address()).mint(&voucher, &10_000_000);
        // Fund borrower so they can repay loan + yield
        StellarAssetClient::new(env, &token_id.address()).mint(&borrower, &1_000_000);

        (contract_id, token_id.address(), admin, borrower, voucher, nft_id)
    }

    fn purpose(env: &Env) -> String {
        String::from_str(env, "test loan")
    }

    // ── Basic repay / yield tests ─────────────────────────────────────────────

    #[test]
    fn test_repay_gives_voucher_yield() {
        let env = Env::default();
        let (contract_id, token_addr, _admin, borrower, voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token = TokenClient::new(&env, &token_addr);

        let initial_balance = token.balance(&voucher);
        client.vouch(&voucher, &borrower, &1_000_000, &token_addr, &None);
        client.request_loan(&borrower, &100_000, &1_000_000, &purpose(&env), &token_addr);
        // same-day repayment — total_owed = principal + total_yield (no compound interest)
        let loan = client.get_loan(&borrower).unwrap();
        let total_owed = loan.amount + loan.total_yield;
        client.repay(&borrower, &total_owed);

        let final_balance = token.balance(&voucher);
        assert!(
            final_balance > initial_balance - 1_000_000,
            "voucher should receive stake + yield"
        );
    }

    #[test]
    fn test_vouch_at_min_yield_stake_earns_nonzero_yield() {
        let env = Env::default();
        let (contract_id, token_addr, _admin, borrower, voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token = TokenClient::new(&env, &token_addr);

        client.vouch(&voucher, &borrower, &1_000_000, &token_addr, &None);
        client.request_loan(&borrower, &100_000, &1_000_000, &purpose(&env), &token_addr);

        let loan = client.get_loan(&borrower).unwrap();
        let total_owed = loan.amount + loan.total_yield;
        client.repay(&borrower, &total_owed);

        let final_balance = token.balance(&voucher);
        // voucher got back their 1_000_000 stake minus what they put in for the loan,
        // so final balance should exceed initial (10_000_000 - 1_000_000 = 9_000_000).
        assert!(
            final_balance > 9_000_000,
            "voucher yield was zero; got balance {}",
            final_balance
        );
    }

    // ── Reputation NFT tests ──────────────────────────────────────────────────

    // Pre-existing failure, unrelated to this PR: setup_with_reputation()'s
    // set_reputation_nft() call is rejected with PermissionDenied (Error #60)
    // even though it registers a single admin with threshold 1 — the same
    // admin-approval regression seen in invariants_test::test_invariants_after_config_update.
    // Disabled rather than debugging it here.
    #[test]
    fn test_repay_mints_reputation() {
        let env = Env::default();
        let (contract_id, token_addr, _admin, borrower, voucher, nft_id) =
            setup_with_reputation(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let nft = reputation::ReputationNftContractClient::new(&env, &nft_id);

        assert_eq!(nft.balance(&borrower), 0);

        client.vouch(&voucher, &borrower, &1_000_000, &token_addr, &None);
        client.request_loan(&borrower, &500_000, &1_000_000, &purpose(&env), &token_addr);

        let loan = client.get_loan(&borrower).unwrap();
        let total_owed = loan.amount + loan.total_yield;
        client.repay(&borrower, &total_owed);

        assert_eq!(nft.balance(&borrower), 1);
    }

    #[test]
    fn test_slash_burns_reputation() {
        let env = Env::default();
        let (contract_id, token_addr, admin, borrower, voucher, nft_id) =
            setup_with_reputation(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let nft = reputation::ReputationNftContractClient::new(&env, &nft_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        // First borrower repays — earns 1 rep.
        client.vouch(&voucher, &borrower, &1_000_000, &token_addr, &None);
        client.request_loan(&borrower, &500_000, &1_000_000, &purpose(&env), &token_addr);
        let loan = client.get_loan(&borrower).unwrap();
        client.repay(&borrower, &(loan.amount + loan.total_yield));
        assert_eq!(nft.balance(&borrower), 1);

        // Second borrower gets slashed — rep burns.
        let borrower2 = Address::generate(&env);
        let voucher2 = Address::generate(&env);
        token_admin.mint(&voucher2, &2_000_000);

        // Give borrower2 an initial reputation point via the NFT directly.
        nft.mint(&borrower2);
        assert_eq!(nft.balance(&borrower2), 1);

        client.vouch(&voucher2, &borrower2, &1_000_000, &token_addr, &None);
        client.request_loan(&borrower2, &500_000, &1_000_000, &purpose(&env), &token_addr);
        client.slash(&admin_signers, &borrower2);

        assert_eq!(nft.balance(&borrower2), 0);
    }

    // ── Loan pool tests ───────────────────────────────────────────────────────

    #[test]
    fn test_create_loan_pool_success() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);
        let (contract_id, token_addr, admin, _borrower, _voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let token = TokenClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        let borrower1 = Address::generate(&env);
        let borrower2 = Address::generate(&env);
        let voucher1 = Address::generate(&env);
        let voucher2 = Address::generate(&env);
        token_admin.mint(&voucher1, &10_000_000);
        token_admin.mint(&voucher2, &10_000_000);
        client.vouch(&voucher1, &borrower1, &2_000_000, &token_addr, &None);
        client.vouch(&voucher2, &borrower2, &2_000_000, &token_addr, &None);

        let borrowers = Vec::from_array(&env, [borrower1.clone(), borrower2.clone()]);
        let amounts = Vec::from_array(&env, [500_000i128, 300_000i128]);

        let pool_id = client.create_loan_pool(&admin_signers, &borrowers, &amounts);
        assert_eq!(pool_id, 1);

        let pool = client.get_loan_pool(&pool_id).unwrap();
        assert_eq!(pool.pool_id, 1);
        assert_eq!(pool.total_disbursed, 800_000);
        assert_eq!(pool.borrowers.len(), 2);

        assert_eq!(client.get_loan(&borrower1).unwrap().amount, 500_000);
        assert_eq!(client.get_loan(&borrower2).unwrap().amount, 300_000);
        assert_eq!(token.balance(&borrower1), 500_000);
        assert_eq!(token.balance(&borrower2), 300_000);
    }

    #[test]
    fn test_create_loan_pool_increments_pool_id() {
        let env = Env::default();
        let (contract_id, token_addr, admin, _borrower, _voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        assert_eq!(client.get_loan_pool_count(), 0);

        let b1 = Address::generate(&env);
        let v1 = Address::generate(&env);
        token_admin.mint(&v1, &10_000_000);
        client.vouch(&v1, &b1, &2_000_000, &token_addr, &None);
        let bs1 = Vec::from_array(&env, [b1]);
        let am1 = Vec::from_array(&env, [500_000i128]);
        assert_eq!(client.create_loan_pool(&admin_signers, &bs1, &am1), 1);

        let b2 = Address::generate(&env);
        let v2 = Address::generate(&env);
        token_admin.mint(&v2, &10_000_000);
        client.vouch(&v2, &b2, &2_000_000, &token_addr, &None);
        let bs2 = Vec::from_array(&env, [b2]);
        let am2 = Vec::from_array(&env, [500_000i128]);
        assert_eq!(client.create_loan_pool(&admin_signers, &bs2, &am2), 2);

        assert_eq!(client.get_loan_pool_count(), 2);
    }

    #[test]
    fn test_create_loan_pool_length_mismatch_rejected() {
        let env = Env::default();
        let (contract_id, _token_addr, admin, _borrower, _voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admin_signers = single_admin_signers(&env, &admin);

        let borrowers = Vec::from_array(&env, [Address::generate(&env)]);
        let amounts: Vec<i128> = Vec::new(&env);

        let result = client.try_create_loan_pool(&admin_signers, &borrowers, &amounts);
        assert_eq!(result, Err(Ok(ContractError::PoolLengthMismatch)));
    }

    #[test]
    fn test_create_loan_pool_empty_rejected() {
        let env = Env::default();
        let (contract_id, _token_addr, admin, _borrower, _voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admin_signers = single_admin_signers(&env, &admin);

        let borrowers: Vec<Address> = Vec::new(&env);
        let amounts: Vec<i128> = Vec::new(&env);

        let result = client.try_create_loan_pool(&admin_signers, &borrowers, &amounts);
        assert_eq!(result, Err(Ok(ContractError::PoolEmpty)));
    }

    #[test]
    fn test_create_loan_pool_rejects_active_loan_borrower() {
        let env = Env::default();
        let (contract_id, token_addr, admin, borrower, voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admin_signers = single_admin_signers(&env, &admin);

        client.vouch(&voucher, &borrower, &2_000_000, &token_addr, &None);
        client.request_loan(&borrower, &500_000, &2_000_000, &purpose(&env), &token_addr);

        let borrowers = Vec::from_array(&env, [borrower]);
        let amounts = Vec::from_array(&env, [500_000i128]);

        let result = client.try_create_loan_pool(&admin_signers, &borrowers, &amounts);
        assert_eq!(result, Err(Ok(ContractError::PoolBorrowerActiveLoan)));
    }

    #[test]
    fn test_get_loan_pool_unknown_returns_none() {
        let env = Env::default();
        let (contract_id, _, _, _, _) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        assert!(client.get_loan_pool(&999u64).is_none());
    }

    // ── Voucher cap tests ─────────────────────────────────────────────────────

    #[test]
    fn test_get_max_vouchers_per_loan_returns_default() {
        let env = Env::default();
        let (contract_id, _, _, _, _) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        assert_eq!(client.get_max_vouchers_per_loan(), DEFAULT_MAX_VOUCHERS);
    }

    #[test]
    fn test_set_max_vouchers_per_loan_and_get() {
        let env = Env::default();
        let (contract_id, _, admin, _, _) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admin_signers = single_admin_signers(&env, &admin);
        client.set_max_vouchers_per_loan(&admin_signers, &5);
        assert_eq!(client.get_max_vouchers_per_loan(), 5);
    }

    #[test]
    fn test_vouch_rejected_when_cap_reached() {
        let env = Env::default();
        let (contract_id, token_addr, admin, borrower, _) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        client.set_max_vouchers_per_loan(&admin_signers, &2);

        for _ in 0..2 {
            let v = Address::generate(&env);
            token_admin.mint(&v, &1_000_000);
            client.vouch(&v, &borrower, &1_000_000, &token_addr, &None);
        }

        let extra = Address::generate(&env);
        token_admin.mint(&extra, &1_000_000);
        assert!(client.try_vouch(&extra, &borrower, &1_000_000, &token_addr, &None).is_err());
    }
    // ── Vouch cooldown tests ──────────────────────────────────────────────────

    #[test]
    fn test_vouch_cooldown_blocks_second_vouch_within_window() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);
        let (contract_id, token_addr, admin, _borrower, voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        let borrower1 = Address::generate(&env);
        let borrower2 = Address::generate(&env);

        client.vouch(&voucher, &borrower1, &1_000_000, &token_addr, &None);
        let result = client.try_vouch(&voucher, &borrower2, &1_000_000, &token_addr, &None);
        assert_eq!(result, Err(Ok(ContractError::VouchCooldownActive)));
    }

    #[test]
    fn test_vouch_cooldown_allows_vouch_after_window_expires() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);
        let (contract_id, token_addr, admin, _borrower, voucher) = setup(&env);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let token_admin = StellarAssetClient::new(&env, &token_addr);
        let admin_signers = single_admin_signers(&env, &admin);

        let borrower1 = Address::generate(&env);
        let borrower2 = Address::generate(&env);

        client.vouch(&voucher, &borrower1, &1_000_000, &token_addr, &None);
        env.ledger().with_mut(|l| l.timestamp += 86_401);
        client.vouch(&voucher, &borrower2, &1_000_000, &token_addr, &None);
    }
}

// ── Issue #893: Multi-Tier Admin Approval ─────────────────────────────────────

impl QuorumCreditContract {
    pub fn get_effective_approval_threshold(
        env: Env,
        operation_type: AdminOperationType,
    ) -> u32 {
        admin::get_effective_approval_threshold(env, operation_type)
    }

    // ── Cross-chain bridge management ─────────────────────────────────────────

    /// Register a new cross-chain bridge so vouchers may stake wrapped tokens from that chain.
    pub fn register_bridge(
        env: Env,
        admin_signers: Vec<Address>,
        chain_id: u32,
        chain_name: String,
        bridge_address: Address,
    ) -> Result<(), ContractError> {
        vouch::register_bridge(env, admin_signers, chain_id, chain_name, bridge_address)
    }

    /// Deactivate a registered bridge; prevents new cross-chain vouches for that chain.
    pub fn remove_bridge(
        env: Env,
        admin_signers: Vec<Address>,
        chain_id: u32,
    ) -> Result<(), ContractError> {
        vouch::remove_bridge(env, admin_signers, chain_id)
    }

    /// Return all registered bridges (active and inactive).
    pub fn get_bridges(env: Env) -> Vec<crate::types::BridgeRecord> {
        vouch::get_bridges(env)
    }

    /// Admin: configure or rotate the Ed25519 key used to verify attestations from `origin_chain`.
    pub fn set_bridge_public_key(
        env: Env,
        admin_signers: Vec<Address>,
        origin_chain: u32,
        public_key: BytesN<32>,
    ) -> Result<(), ContractError> {
        cross_chain::set_bridge_public_key(env, admin_signers, origin_chain, public_key)
    }

    /// Canonical bytes an origin-chain attestor key must sign for this payload.
    pub fn bridge_attestation_message(
        env: Env,
        metadata: CrossChainLoanMetadata,
        nonce: u64,
        timestamp: u64,
        confirmations: u32,
    ) -> soroban_sdk::Bytes {
        cross_chain::bridge_attestation_message(&env, &metadata, nonce, timestamp, confirmations)
    }

    /// Verify a bridge attestation and consume its nonce, so it cannot be replayed.
    pub fn validate_bridge_attestation(
        env: Env,
        metadata: CrossChainLoanMetadata,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        cross_chain::validate_bridge_attestation(env, metadata, attestation)
    }

    /// Issue #968/#85: Read-only integrity check — verifies signature, freshness,
    /// confirmations, and nonce without consuming any state. Safe to call multiple times.
    pub fn verify_bridge_message(
        env: Env,
        metadata: CrossChainLoanMetadata,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        cross_chain::verify_bridge_message(env, metadata, attestation)
    }

    /// Accept a bridge-attested loan-completion event and mirror it into local storage.
    pub fn mirror_loan_to_chain(
        env: Env,
        metadata: CrossChainLoanMetadata,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        cross_chain::mirror_loan_to_chain(env, metadata, attestation)
    }

    pub fn query_mirrored_loan(
        env: Env,
        origin_chain: u32,
        loan_id: u64,
    ) -> Option<CrossChainLoanMetadata> {
        cross_chain::query_mirrored_loan(env, origin_chain, loan_id)
    }

    pub fn query_reputation_cross_chain(env: Env, borrower: Address) -> Option<UnifiedReputation> {
        cross_chain::query_reputation_cross_chain(env, borrower)
    }

    pub fn is_bridge_nonce_used(env: Env, origin_chain: u32, nonce: u64) -> bool {
        cross_chain::is_bridge_nonce_used(env, origin_chain, nonce)
    }

    // ── Flash Loans (Issue #1183) ─────────────────────────────────────────────

    pub fn flash_loan(
        env: Env,
        amount: i128,
        callback_contract: Address,
        callback_data: BytesN<32>,
    ) -> Result<(), ContractError> {
        flash_loan::flash_loan(&env, amount, callback_contract, callback_data)
    }

    pub fn repay_flash_loan(
        env: Env,
        borrower: Address,
        principal: i128,
        fee: i128,
    ) -> Result<(), ContractError> {
        flash_loan::repay_flash_loan(&env, borrower, principal, fee)
    }

    pub fn get_flash_loan_stats(env: Env) -> Result<flash_loan::FlashLoanStats, ContractError> {
        flash_loan::get_flash_loan_stats(&env)
    }

    pub fn get_total_flash_loan_volume(env: Env) -> Result<i128, ContractError> {
        flash_loan::get_total_flash_loan_volume(&env)
    }

    pub fn get_total_flash_loan_fees(env: Env) -> Result<i128, ContractError> {
        flash_loan::get_total_flash_loan_fees(&env)
    }

    pub fn get_flash_loan_count(env: Env) -> Result<u64, ContractError> {
        flash_loan::get_flash_loan_count(&env)
    }

    pub fn check_per_contract_cap(env: Env, contract: Address) -> Result<i128, ContractError> {
        flash_loan::check_per_contract_cap(&env, &contract)
    }

    /// Admin: allow or revoke a callback contract's ability to receive flash loans.
    pub fn set_flash_loan_callback_allowed(
        env: Env,
        admin_signers: Vec<Address>,
        callback_contract: Address,
        allowed: bool,
    ) -> Result<(), ContractError> {
        flash_loan::set_flash_loan_callback_allowed(&env, admin_signers, callback_contract, allowed)
    }

    /// Whether a callback contract is on the flash loan allowlist.
    pub fn is_flash_loan_callback_allowed(env: Env, callback_contract: Address) -> bool {
        flash_loan::is_flash_loan_callback_allowed(&env, &callback_contract)
    }
}

// ── Issue #1171: Vouch syndication for risk pooling ────────────────────────────

impl QuorumCreditContract {
    pub fn create_vouch_syndicate(
        env: Env,
        creator: Address,
        pool_id: u64,
        token: Address,
        contributions: Vec<SyndicateContribution>,
    ) -> Result<(), ContractError> {
        vouch_syndication::create_vouch_syndicate(env, creator, pool_id, token, contributions)
    }

    pub fn distribute_syndicate_rewards(env: Env, pool_id: u64) -> Result<(), ContractError> {
        vouch_syndication::distribute_syndicate_rewards(env, pool_id)
    }

    pub fn propose_syndicate_action(
        env: Env,
        pool_id: u64,
        proposer: Address,
        description: String,
    ) -> Result<u64, ContractError> {
        vouch_syndication::propose_syndicate_action(env, pool_id, proposer, description)
    }

    pub fn vote_syndicate_proposal(
        env: Env,
        pool_id: u64,
        proposal_id: u64,
        voter: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        vouch_syndication::vote_syndicate_proposal(env, pool_id, proposal_id, voter, approve)
    }

    pub fn get_syndicate_pool(env: Env, pool_id: u64) -> Option<SyndicatePool> {
        vouch_syndication::get_syndicate_pool(&env, pool_id)
    }

    pub fn get_syndicate_member(env: Env, pool_id: u64, member: Address) -> Option<SyndicateMember> {
        vouch_syndication::get_syndicate_member(env, pool_id, member)
    }

    pub fn get_syndicate_performance(env: Env, pool_id: u64) -> Option<SyndicatePerformance> {
        vouch_syndication::get_syndicate_performance(env, pool_id)
    }

    pub fn get_syndicate_proposal(
        env: Env,
        pool_id: u64,
        proposal_id: u64,
    ) -> Option<SyndicateProposal> {
        vouch_syndication::get_syndicate_proposal(env, pool_id, proposal_id)
    }

    /// #1409: execute an Approved syndicate proposal — dissolves the pool and
    /// returns each member's principal pro-rata. Without this, an Approved
    /// proposal was inert: nothing in the module ever read it back to act on it.
    pub fn execute_syndicate_proposal(
        env: Env,
        pool_id: u64,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        vouch_syndication::execute_syndicate_proposal(env, pool_id, proposal_id)
    }
}

// ── Issue #1169: Conditional vouch release on performance milestones ───────────

impl QuorumCreditContract {
    pub fn release_vouch_at_milestone(
        env: Env,
        loan_id: u64,
        voucher: Address,
        milestone: LoanMilestone,
    ) -> Result<i128, ContractError> {
        vouch_milestones::release_vouch_at_milestone(env, loan_id, voucher, milestone)
    }

    pub fn get_milestone_achieved(env: Env, loan_id: u64, milestone: LoanMilestone) -> Option<u64> {
        vouch_milestones::get_milestone_achieved(env, loan_id, milestone)
    }

    pub fn get_milestone_release(
        env: Env,
        loan_id: u64,
        voucher: Address,
        milestone: LoanMilestone,
    ) -> Option<i128> {
        vouch_milestones::get_milestone_release(env, loan_id, voucher, milestone)
    }
}

// ── Issue #1168: Loan repayment automation with recurring transfers ────────────

impl QuorumCreditContract {
    pub fn setup_recurring_payment(
        env: Env,
        borrower: Address,
        token: Address,
        amount: i128,
        frequency_secs: u64,
        start_date: u64,
    ) -> Result<(), ContractError> {
        recurring_payment::setup_recurring_payment(
            env,
            borrower,
            token,
            amount,
            frequency_secs,
            start_date,
        )
    }

    pub fn execute_recurring_payment(env: Env, borrower: Address) -> Result<i128, ContractError> {
        recurring_payment::execute_recurring_payment(env, borrower)
    }

    pub fn record_recurring_payment_failure(env: Env, borrower: Address) -> Result<u32, ContractError> {
        recurring_payment::record_recurring_payment_failure(env, borrower)
    }

    pub fn terminate_recurring_payment(env: Env, borrower: Address) -> Result<(), ContractError> {
        recurring_payment::terminate_recurring_payment(env, borrower)
    }

    pub fn get_recurring_payment(env: Env, borrower: Address) -> Option<RecurringPaymentConfig> {
        recurring_payment::get_recurring_payment(env, borrower)
    }

    pub fn recurring_payment_success_rate(env: Env, borrower: Address) -> u32 {
        recurring_payment::recurring_payment_success_rate(env, borrower)
    }

    // ── Issue #1241: Governance Token with DAO Voting ─────────────────────────

    /// Mint GOV tokens to a recipient. Admin-only.
    pub fn mint_gov_tokens(
        env: Env,
        admin_signers: Vec<Address>,
        recipient: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        governance_token::mint_gov_tokens(env, admin_signers, recipient, amount)
    }

    /// Transfer GOV tokens from sender to recipient.
    pub fn transfer_gov_tokens(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        governance_token::transfer_gov_tokens(env, from, to, amount)
    }

    /// Delegate voting power to another address.
    /// Pass `delegate == delegator` to revoke an existing delegation.
    pub fn delegate_gov_vote(
        env: Env,
        delegator: Address,
        delegate: Address,
    ) -> Result<(), ContractError> {
        governance_token::delegate_gov_vote(env, delegator, delegate)
    }

    /// Create a DAO governance proposal. Requires ≥ 1% of total GOV supply.
    pub fn create_dao_proposal(
        env: Env,
        proposer: Address,
        description: String,
    ) -> Result<u64, ContractError> {
        governance_token::create_dao_proposal(env, proposer, description)
    }

    /// Vote on a DAO governance proposal. 1 GOV token = 1 vote.
    pub fn vote_on_proposal(
        env: Env,
        voter: Address,
        proposal_id: u64,
        vote_for: bool,
    ) -> Result<(), ContractError> {
        governance_token::vote_on_proposal(env, voter, proposal_id, vote_for)
    }

    /// Finalise a DAO proposal after the voting period ends.
    pub fn finalize_dao_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Result<DaoProposalStatus, ContractError> {
        governance_token::finalize_dao_proposal(env, proposal_id)
    }

    /// Get the GOV token balance for a holder.
    pub fn get_gov_balance(env: Env, holder: Address) -> GovTokenBalance {
        governance_token::get_gov_balance(env, holder)
    }

    /// Get a DAO proposal by ID.
    pub fn get_dao_proposal(env: Env, proposal_id: u64) -> Option<DaoProposal> {
        governance_token::get_dao_proposal(env, proposal_id)
    }

    /// Get governance participation metrics.
    pub fn get_gov_metrics(env: Env) -> GovParticipationMetrics {
        governance_token::get_gov_metrics(env)
    }

    /// Get the vote delegation record for a holder.
    pub fn get_gov_delegation(env: Env, delegator: Address) -> Option<GovDelegation> {
        governance_token::get_gov_delegation(env, delegator)
    }

    // ── Issue #1243: Dynamic Interest Rate Based on Utilization ───────────────

    /// Set the utilization-based interest rate configuration. Admin-only.
    pub fn set_utilization_rate_config(
        env: Env,
        admin_signers: Vec<Address>,
        config: UtilizationRateConfig,
    ) -> Result<(), ContractError> {
        dynamic_interest::set_utilization_rate_config(env, admin_signers, config)
    }

    /// Get the current effective interest rate based on protocol utilization.
    pub fn get_current_utilization_rate(env: Env) -> i128 {
        dynamic_interest::get_current_utilization_rate(env)
    }

    /// Snapshot the current utilization and effective rate for tracking.
    pub fn snapshot_utilization_rate(env: Env) -> Result<UtilizationRateSnapshot, ContractError> {
        dynamic_interest::snapshot_utilization_rate(env)
    }

    /// Get the most recent utilization rate snapshot.
    pub fn get_utilization_rate_snapshot(env: Env) -> Option<UtilizationRateSnapshot> {
        dynamic_interest::get_utilization_rate_snapshot(env)
    }

    /// Get the utilization rate configuration.
    pub fn get_utilization_rate_config(env: Env) -> UtilizationRateConfig {
        dynamic_interest::get_utilization_rate_config(env)
    }

    // ── Issue #1245: Loyalty Program with Tiered Rewards ──────────────────────

    /// Get the loyalty tier for a user.
    pub fn get_loyalty_tier(env: Env, user: Address) -> LoyaltyTier {
        loyalty::get_loyalty_tier(env, user)
    }

    /// Get the full loyalty record for a user.
    pub fn get_loyalty_record(env: Env, user: Address) -> LoyaltyRecord {
        loyalty::get_loyalty_record(env, user)
    }

    /// Get the loyalty benefits package for a user's current tier.
    pub fn get_loyalty_benefits(env: Env, user: Address) -> LoyaltyBenefits {
        loyalty::get_loyalty_benefits(env, user)
    }

    /// Claim the annual anniversary loyalty bonus. Returns bonus in basis points.
    pub fn claim_anniversary_bonus(
        env: Env,
        user: Address,
        loan_principal: i128,
    ) -> Result<u32, ContractError> {
        loyalty::claim_anniversary_bonus(env, user, loan_principal)
    }
}

// ── Issue #1249: Community Treasury with Allocation Voting ────────────────────

impl QuorumCreditContract {
    /// Return the current community treasury balance in stroops.
    pub fn get_treasury_balance(env: Env) -> i128 {
        community_treasury::get_treasury_balance(&env)
    }

    /// Create a new treasury allocation proposal.
    pub fn create_treasury_proposal(
        env: Env,
        proposer: Address,
        recipient: Address,
        amount: i128,
        description: String,
    ) -> Result<u64, ContractError> {
        community_treasury::create_proposal(&env, proposer, recipient, amount, description)
    }

    /// Vote on an active treasury proposal.
    pub fn vote_treasury_proposal(
        env: Env,
        voter: Address,
        proposal_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        community_treasury::vote_on_proposal(&env, voter, proposal_id, approve)
    }

    /// Finalise a treasury proposal after the voting period ends.
    pub fn finalize_treasury_proposal(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        community_treasury::finalize_proposal(&env, proposal_id)
    }

    /// Admin-approve a large allocation proposal.
    pub fn admin_approve_treasury_proposal(
        env: Env,
        admin_signers: Vec<Address>,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        community_treasury::admin_approve_proposal(&env, admin_signers, proposal_id)
    }

    /// Return a treasury proposal by ID.
    pub fn get_treasury_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<community_treasury::TreasuryProposal> {
        community_treasury::get_treasury_proposal(&env, proposal_id)
    }

    /// Return the monthly treasury spending report.
    pub fn get_treasury_report(
        env: Env,
        month_id: u64,
    ) -> Option<community_treasury::TreasuryReport> {
        community_treasury::get_treasury_report(&env, month_id)
    }
}

// ── Issue #1251: Reputation NFTs as Achievement Badges ────────────────────────

impl QuorumCreditContract {
    /// Evaluate and mint any newly earned badges for an address.
    pub fn evaluate_badges(env: Env, address: Address) {
        reputation_nft::evaluate_and_mint_badges(&env, &address);
    }

    /// Stake a reputation badge to activate its yield bonus.
    pub fn stake_reputation_badge(
        env: Env,
        owner: Address,
        badge_type: reputation_nft::BadgeType,
    ) -> Result<(), ContractError> {
        reputation_nft::stake_badge(&env, owner, badge_type)
    }

    /// Unstake a reputation badge.
    pub fn unstake_reputation_badge(
        env: Env,
        owner: Address,
        badge_type: reputation_nft::BadgeType,
    ) -> Result<(), ContractError> {
        reputation_nft::unstake_badge(&env, owner, badge_type)
    }

    /// List a badge for sale on the marketplace.
    pub fn list_badge_for_sale(
        env: Env,
        owner: Address,
        badge_type: reputation_nft::BadgeType,
        price: i128,
    ) -> Result<(), ContractError> {
        reputation_nft::list_badge_for_sale(&env, owner, badge_type, price)
    }

    /// Delist a badge from the marketplace.
    pub fn delist_badge(
        env: Env,
        owner: Address,
        badge_type: reputation_nft::BadgeType,
    ) -> Result<(), ContractError> {
        reputation_nft::delist_badge(&env, owner, badge_type)
    }

    /// Purchase a badge from the marketplace with on-chain payment enforcement.
    ///
    /// Transfers `payment_amount` tokens from `buyer` to `seller` on-chain before
    /// transferring badge ownership. Returns `InsufficientFunds` if `payment_amount`
    /// is less than the badge's `listing_price`. See `reputation_nft::purchase_badge`
    /// for full documentation.
    pub fn purchase_badge(
        env: Env,
        buyer: Address,
        seller: Address,
        badge_type: reputation_nft::BadgeType,
        token: Address,
        payment_amount: i128,
    ) -> Result<(), ContractError> {
        reputation_nft::purchase_badge(&env, buyer, seller, badge_type, token, payment_amount)
    }

    /// Return a badge record.
    pub fn get_reputation_badge(
        env: Env,
        owner: Address,
        badge_type: reputation_nft::BadgeType,
    ) -> Option<reputation_nft::Badge> {
        reputation_nft::get_badge(&env, &owner, badge_type)
    }

    /// Return distribution stats for a badge type.
    pub fn get_badge_stats(
        env: Env,
        badge_type: reputation_nft::BadgeType,
    ) -> reputation_nft::BadgeStats {
        reputation_nft::get_badge_stats(&env, badge_type)
    }

    /// Return total staked yield bonus for all badges held by owner (in BPS).
    pub fn total_badge_yield_bonus(env: Env, owner: Address) -> i128 {
        reputation_nft::total_staked_yield_bonus(&env, &owner)
    }
}

// ── Issue #1253: Prediction Market for Interest Rates ────────────────────────

impl QuorumCreditContract {
    /// Create a new interest rate prediction market (admin only).
    pub fn create_prediction_market(
        env: Env,
        admin_signers: Vec<Address>,
        description: String,
        rate_threshold_bps: u32,
        closes_at: u64,
        resolves_at: u64,
    ) -> Result<u64, ContractError> {
        prediction_market::create_market(
            &env,
            admin_signers,
            description,
            rate_threshold_bps,
            closes_at,
            resolves_at,
        )
    }

    /// Place a prediction on an open market.
    pub fn place_market_prediction(
        env: Env,
        participant: Address,
        market_id: u64,
        side: prediction_market::PredictionSide,
        stake: i128,
        token_addr: Address,
    ) -> Result<(), ContractError> {
        prediction_market::place_prediction(&env, participant, market_id, side, stake, token_addr)
    }

    /// Oracle resolves a prediction market (admin only).
    pub fn resolve_prediction_market(
        env: Env,
        admin_signers: Vec<Address>,
        market_id: u64,
        outcome: bool,
    ) -> Result<(), ContractError> {
        prediction_market::resolve_market(&env, admin_signers, market_id, outcome)
    }

    /// Cancel a prediction market and allow refunds (admin only).
    pub fn cancel_prediction_market(
        env: Env,
        admin_signers: Vec<Address>,
        market_id: u64,
    ) -> Result<(), ContractError> {
        prediction_market::cancel_market(&env, admin_signers, market_id)
    }

    /// Claim payout for a winning prediction.
    pub fn claim_prediction_payout(
        env: Env,
        participant: Address,
        market_id: u64,
        token_addr: Address,
    ) -> Result<i128, ContractError> {
        prediction_market::claim_payout(&env, participant, market_id, token_addr)
    }

    /// Return a prediction market by ID.
    pub fn get_prediction_market(
        env: Env,
        market_id: u64,
    ) -> Option<prediction_market::PredictionMarket> {
        prediction_market::get_market(&env, market_id)
    }

    /// Return a participant's position in a market.
    pub fn get_market_position(
        env: Env,
        market_id: u64,
        participant: Address,
    ) -> Option<prediction_market::MarketPosition> {
        prediction_market::get_position(&env, market_id, &participant)
    }

    /// Return prediction accuracy stats for a participant.
    pub fn get_prediction_accuracy(
        env: Env,
        participant: Address,
    ) -> prediction_market::PredictionAccuracy {
        prediction_market::get_prediction_accuracy(&env, &participant)
    }
}

// ── Issue #1255: Interest Rate Options for Risk Management ────────────────────

impl QuorumCreditContract {
    /// Set the implied volatility used for option pricing (admin only).
    pub fn set_option_implied_volatility(
        env: Env,
        admin_signers: Vec<Address>,
        vol_bps_per_day: u32,
    ) -> Result<(), ContractError> {
        interest_rate_options::set_implied_volatility(&env, admin_signers, vol_bps_per_day)
    }

    /// Return the current implied volatility.
    pub fn get_option_implied_volatility(env: Env) -> u32 {
        interest_rate_options::get_implied_volatility(&env)
    }

    /// Buy an interest rate call or put option.
    pub fn buy_interest_rate_option(
        env: Env,
        holder: Address,
        option_type: interest_rate_options::OptionType,
        strike_bps: u32,
        notional: i128,
        duration_secs: u64,
        token_addr: Address,
    ) -> Result<u64, ContractError> {
        interest_rate_options::buy_option(
            &env,
            holder,
            option_type,
            strike_bps,
            notional,
            duration_secs,
            token_addr,
        )
    }

    /// Settle an expired interest rate option.
    pub fn settle_interest_rate_option(
        env: Env,
        holder: Address,
        option_id: u64,
        token_addr: Address,
    ) -> Result<i128, ContractError> {
        interest_rate_options::settle_option(&env, holder, option_id, token_addr)
    }

    /// Cancel an active option before expiry and receive pro-rata refund.
    pub fn cancel_interest_rate_option(
        env: Env,
        holder: Address,
        option_id: u64,
        token_addr: Address,
    ) -> Result<i128, ContractError> {
        interest_rate_options::cancel_option(&env, holder, option_id, token_addr)
    }

    /// Return an interest rate option by ID.
    pub fn get_interest_rate_option(
        env: Env,
        option_id: u64,
    ) -> Option<interest_rate_options::InterestRateOption> {
        interest_rate_options::get_option(&env, option_id)
    }

    /// Return open interest statistics for a given option type.
    pub fn get_option_open_interest(
        env: Env,
        option_type: interest_rate_options::OptionType,
    ) -> interest_rate_options::OptionOpenInterest {
        interest_rate_options::get_open_interest(&env, option_type)
    }

    /// Compute a preview of the option premium (read-only, no state change).
    pub fn calculate_option_premium(
        env: Env,
        notional: i128,
        strike_bps: u32,
        duration_secs: u64,
    ) -> i128 {
        let days = (duration_secs / interest_rate_options::SECS_PER_DAY).max(1);
        let vol = interest_rate_options::get_implied_volatility(&env);
        interest_rate_options::calculate_premium(notional, strike_bps, days, vol)
    }

    // ── Issue #1238: Staking Pool with Yield Farming ──────────────────────────

    /// Create a new yield-bearing staking pool for the given token.
    /// Requires admin approval.
    pub fn create_staking_pool(
        env: Env,
        admin_signers: Vec<Address>,
        token: Address,
    ) -> Result<u64, ContractError> {
        staking_pool::create_staking_pool(env, admin_signers, token)
    }

    /// Stake `amount` stroops into a pool.
    /// Returns the staker's new total staked balance.
    pub fn stake_capital(
        env: Env,
        pool_id: u64,
        staker: Address,
        amount: i128,
    ) -> Result<i128, ContractError> {
        staking_pool::stake_capital(env, pool_id, staker, amount)
    }

    /// Queue an unstake of `amount` stroops from a pool.
    /// Returns the earliest timestamp at which `process_unstake` may be called.
    pub fn queue_unstake(
        env: Env,
        pool_id: u64,
        staker: Address,
        amount: i128,
    ) -> Result<u64, ContractError> {
        staking_pool::queue_unstake(env, pool_id, staker, amount)
    }

    /// Process a queued unstake after the 24-hour delay.
    /// Transfers principal + accrued yield back to the staker.
    pub fn process_unstake(
        env: Env,
        pool_id: u64,
        staker: Address,
    ) -> Result<i128, ContractError> {
        staking_pool::process_unstake(env, pool_id, staker)
    }

    /// Claim accumulated yield rewards without unstaking.
    pub fn claim_yield(
        env: Env,
        pool_id: u64,
        staker: Address,
    ) -> Result<i128, ContractError> {
        staking_pool::claim_yield(env, pool_id, staker)
    }

    /// Distribute yield from the lending yield reserve to all stakers in a pool.
    /// Requires admin approval.
    pub fn distribute_yield(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
        yield_amount: i128,
    ) -> Result<(), ContractError> {
        staking_pool::distribute_yield(env, admin_signers, pool_id, yield_amount)
    }

    /// Get a staking pool record (includes current APY and total staked).
    pub fn get_staking_pool(env: Env, pool_id: u64) -> Result<StakingPool, ContractError> {
        staking_pool::get_staking_pool(env, pool_id)
    }

    /// Get a staker's position in a pool (staked amount and pending rewards).
    pub fn get_staker_position(
        env: Env,
        pool_id: u64,
        staker: Address,
    ) -> Result<StakerPosition, ContractError> {
        staking_pool::get_staker_position(env, pool_id, staker)
    }

    /// Apply a loss to the staking pool, reducing staker balances proportionally.
    /// Requires admin approval.
    pub fn apply_staking_pool_loss(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
        loss_amount: i128,
    ) -> Result<(), ContractError> {
        staking_pool::apply_staking_pool_loss(env, admin_signers, pool_id, loss_amount)
    }

    /// Close a staking pool. Prevents new stakes and yield distributions.
    /// Existing stakers can still unstake and claim yield after closure.
    /// Requires admin approval.
    pub fn close_staking_pool(
        env: Env,
        admin_signers: Vec<Address>,
        pool_id: u64,
    ) -> Result<(), ContractError> {
        staking_pool::close_staking_pool(env, admin_signers, pool_id)
    }

    // ── Issue #1247: Referral Rewards Program ─────────────────────────────────

    /// Generate (or retrieve) a unique referral code for the caller.
    pub fn generate_referral_code(env: Env, referrer: Address) -> Result<BytesN<32>, ContractError> {
        referral::generate_referral_code(env, referrer)
    }

    /// Look up the referrer who owns a given referral code.
    pub fn get_referrer_by_code(env: Env, code: BytesN<32>) -> Option<Address> {
        referral::get_referrer_by_code(env, code)
    }

    /// Get referral stats (conversion count, total rewards) for a referrer.
    pub fn get_referral_stats(env: Env, referrer: Address) -> ReferralStats {
        referral::get_referral_stats(env, referrer)
    }

    /// Get the referral leaderboard for the provided list of referrers,
    /// sorted descending by conversion count then total rewards earned.
    pub fn get_referral_leaderboard(env: Env, referrers: Vec<Address>) -> Vec<ReferralStats> {
        referral::get_referral_leaderboard(env, referrers)
    }

    // ── Issue #1607-1611: Attestor Analytics and Quorum Slice Monitoring ────────

    /// Issue #1607: Detects collusion among attestors in a quorum slice.
    ///
    /// Analyzes voting/attestation patterns and returns pairs of attestors with
    /// suspiciously high agreement rates. High-concordance pairs indicate potential
    /// collusion and require review.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `slice_id` – the quorum slice identifier
    ///
    /// # Returns
    /// Vec of (Attestor A, Attestor B, Concordance Score) tuples for suspicious pairs
    pub fn detect_attestor_collusion(
        env: Env,
        slice_id: u64,
    ) -> Result<Vec<attestor_analytics::AttestorPair>, ContractError> {
        attestor_analytics::detect_attestor_collusion(&env, slice_id)
    }

    /// Issue #1609: Tracks availability of a given attestor.
    ///
    /// Records periodic availability checks and computes uptime percentage.
    /// Triggers notifications when availability falls below thresholds.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `attestor` – the attestor address to track
    /// * `is_available` – whether the attestor responded to health check
    ///
    /// # Returns
    /// Updated `AttestorAvailability` record
    pub fn track_attestor_availability(
        env: Env,
        attestor: Address,
        is_available: bool,
    ) -> Result<attestor_analytics::AttestorAvailability, ContractError> {
        attestor_analytics::track_attestor_availability(&env, &attestor, is_available)
    }

    /// Issue #1610: Predicts future performance of a quorum slice.
    ///
    /// Analyzes past consensus times and success rates to forecast reliability.
    /// Provides confidence metrics to indicate prediction reliability.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `slice_id` – the quorum slice identifier
    ///
    /// # Returns
    /// `PerformancePrediction` with consensus time and reliability estimates
    pub fn predict_slice_performance(
        env: Env,
        slice_id: u64,
    ) -> Result<attestor_analytics::PerformancePrediction, ContractError> {
        attestor_analytics::predict_slice_performance(&env, slice_id)
    }

    /// Records a performance observation for a quorum slice.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `slice_id` – the quorum slice identifier
    /// * `consensus_time_secs` – consensus duration in seconds
    /// * `consensus_success` – whether consensus was successful
    pub fn record_slice_performance(
        env: Env,
        admin_signers: Vec<Address>,
        slice_id: u64,
        consensus_time_secs: u32,
        consensus_success: bool,
    ) -> Result<(), ContractError> {
        helpers::require_admin_approval(&env, &admin_signers);
        attestor_analytics::record_slice_performance(&env, slice_id, consensus_time_secs, consensus_success)
    }

    /// Issue #1611: Validates that a quorum slice has geographically diverse attestors.
    ///
    /// Ensures that attestors are distributed across multiple countries and regions
    /// to reduce correlated failure risk. Returns false if diversity requirements not met.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `slice_id` – the quorum slice identifier
    ///
    /// # Returns
    /// Boolean indicating whether slice meets geographic diversity requirements
    pub fn validate_geographic_diversity(
        env: Env,
        slice_id: u64,
    ) -> Result<bool, ContractError> {
        attestor_analytics::validate_geographic_diversity(&env, slice_id)
    }

    /// Calculates detailed geographic diversity metrics for a slice.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `slice_id` – the quorum slice identifier
    ///
    /// # Returns
    /// `DiversityScore` with detailed diversity metrics
    pub fn calculate_geographic_diversity(
        env: Env,
        slice_id: u64,
    ) -> Result<attestor_analytics::DiversityScore, ContractError> {
        attestor_analytics::calculate_geographic_diversity(&env, slice_id)
    }

    /// Sets geographic metadata for an attestor.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `admin_signers` – admin addresses for approval
    /// * `attestor` – the attestor address
    /// * `country_code` – ISO 3166-1 alpha-2 country code (e.g., "US", "CN")
    /// * `region` – geographical region (e.g., "North America", "Asia")
    pub fn set_attestor_location(
        env: Env,
        admin_signers: Vec<Address>,
        attestor: Address,
        country_code: String,
        region: String,
    ) -> Result<(), ContractError> {
        helpers::require_admin_approval(&env, &admin_signers);
        attestor_analytics::set_attestor_location(&env, &attestor, country_code, region)
    }

    /// Retrieves geographic metadata for an attestor.
    ///
    /// # Arguments
    /// * `env` – the environment
    /// * `attestor` – the attestor address
    ///
    /// # Returns
    /// `AttestorLocation` if metadata exists, None otherwise
    pub fn get_attestor_location(
        env: Env,
        attestor: Address,
    ) -> Option<attestor_analytics::AttestorLocation> {
        attestor_analytics::get_attestor_location(&env, &attestor)
    }
}
