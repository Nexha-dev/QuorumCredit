# QuorumCredit Backup and Recovery Guide

## Overview

This guide documents backup and recovery procedures for QuorumCredit contract state and data. Data loss could be catastrophic for borrowers and vouchers, so comprehensive backup strategies are essential.

## What's automated today vs. what still needs an operator (Issue #1146)

**Automated:**
- `scripts/backup.sh` derives the per-borrower snapshot address set from the
  `quorum-credit-indexer`'s recorded event history (every address that has
  ever appeared in a `loan` or `vouch` event), not a manually-maintained list.
  `BORROWER_ADDRESSES` is still supported but only as a supplementary merge.
- The backup manifest (`manifest.json`) includes a **completeness proof**
  (`derived_address_count` vs. the on-chain `get_borrower_count()`) and a
  **checksum** (`sha256:...`) over every backed-up file, so an operator can
  verify nothing was silently skipped or corrupted before trusting a backup.
- `scripts/restore.sh --scenario 6` is a genuinely automated, idempotent, and
  resumable data-loss recovery path: it diffs the backup against live state,
  replays only what's missing, records progress in a state file (safe to kill
  and restart without duplicating work), and is gated by the live
  `check_invariants` contract entrypoint before and after every step.

**Still requires an operator:**
- Scenarios 1-5 (unpause, config restore, yield-reserve check, admin key
  rotation, contract upgrade) remain guided runbooks — `restore.sh` prints
  (or, with `--execute`, runs) each command, but decisions like which admin
  key to trust or when a WASM upgrade is safe to apply are not automatable.
- Reviewing *why* a backup shows `"complete": false` or a `check_invariants`
  failure, and deciding how to remediate, is always a human judgment call.
- Off-chain database backups and cold-storage archival described later in this
   guide are illustrative examples, not scripts shipped in this repo.

## Automated Backup Testing (Issue #1225)

Backups that are never restored are unverified assumptions, not disaster recovery. `scripts/test_backup_restore.sh` closes that gap by running the full backup → restore → verify path on a schedule.

**What it does, every run:**
1. Runs `scripts/backup.sh` against the configured contract.
2. Verifies the resulting manifest: completeness proof (`complete` must not be `false`) and a recomputed checksum match (catches silent corruption/truncation).
3. Executes `scripts/restore.sh --scenario 6` against a **disposable staging contract** (`STAGING_CONTRACT_ID`) that must differ from the source contract. The automated drill fails closed if execution is disabled or the staging target is missing or matches the source; a dry-run is never counted as a successful restore test.
4. When executed, re-queries the staging contract for all sampled borrowers and diffs each against the backed-up values:
   - `get_paused` — contract pause state
   - `get_loan` — loan records for each borrower
   - `get_vouches` — vouch records for each borrower
   - `loan_status` — loan status for each borrower
   - `total_vouched` — total vouched amounts for each borrower
   
   By default, all borrowers present in the backup's `derived_addresses.txt` are sampled. This can be overridden by setting `BACKUP_TEST_SAMPLE_BORROWERS` to point to a custom address list. A restore that "completes" but corrupts, drops, or misattributes any loan/vouch record will be caught as a failure rather than reported as a pass.
5. Appends a result record (pass/fail, failure reason, per-step booleans, duration) to `backups/test-results/history.jsonl` and prints the cumulative coverage/success rate computed from that history.

**Schedule:** `.github/workflows/backup-restore-test.yml` runs this weekly (Sundays 03:00 UTC) and on manual dispatch. Each run downloads the previous verification-history artifact before appending its result, then uploads the cumulative history and backup archive. A failed run opens or updates a GitHub issue titled **Automated backup verification failed**; a later successful run comments on and closes that issue. The workflow requires these repository secrets/variables:

| Name | Kind | Purpose |
|---|---|---|
| `BACKUP_TEST_CONTRACT_ID` / `BACKUP_TEST_ADMIN_KEY` | secret | Contract to back up |
| `BACKUP_TEST_STAGING_CONTRACT_ID` / `BACKUP_TEST_STAGING_ADMIN_KEY` | secret | Disposable staging contract and key; the staging contract must differ from the source |
| `BACKUP_TEST_NETWORK` | variable | `testnet` (default) or the network hosting both contracts |
| `BACKUP_TEST_SAMPLE_BORROWERS` | variable (optional) | Path to a custom borrower address list for Step 4 sampling (defaults to all addresses in the backup) |

The workflow always executes the restore and correctness checks. Pull requests run shell syntax and restore-safety policy tests only; they never receive or use deployment secrets. The GitHub Actions token needs permission to read workflow artifacts and create/update repository issues for history persistence and failure alerts.

**Coverage/success-rate tracking:** every run appends to `backups/test-results/history.jsonl` (one JSON object per line — `status`, `failure_reason`, `completeness`, `checksum_ok`, `restore_ok`, `correctness`, `duration_seconds`). The workflow carries this file forward through its `backup-verification-history` artifact, retained for 90 days. Preflight failures (including missing secrets and unsafe restore targets) are recorded too. The script prints the cumulative `passed/total (success rate %)` summary each time; the file is the source of truth for trend analysis (e.g. `jq -s 'map(.status=="pass") | add / length' backups/test-results/history.jsonl`).

**Still requires an operator:** deciding whether a `false` completeness proof or a correctness mismatch reflects a real data-loss event (vs. an indexer lag or a staging environment quirk) is a judgment call — the drill's job is to make that failure loud and immediate, not to self-diagnose it.

## Contract State Export

### Exporting Contract State

Contract state is stored on the Stellar ledger and can be exported using the Stellar RPC API:

```bash
# Export all contract storage entries
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network testnet \
  --source $ADMIN_KEY

# Export loan records
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_loan \
  --network testnet \
  --source $ADMIN_KEY \
  -- --borrower $BORROWER_ADDRESS

# Export vouches
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_vouches \
  --network testnet \
  --source $ADMIN_KEY \
  -- --borrower $BORROWER_ADDRESS
```

### Automated State Snapshots

Implement automated snapshots using a scheduled job:

```bash
#!/bin/bash
# backup-contract-state.sh

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="./backups/contract_state"
mkdir -p $BACKUP_DIR

# Export config
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network $NETWORK \
  --source $ADMIN_KEY > $BACKUP_DIR/config_$TIMESTAMP.json

# Export admin audit log
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admin_audit_log \
  --network $NETWORK \
  --source $ADMIN_KEY > $BACKUP_DIR/audit_log_$TIMESTAMP.json

# Export slash treasury
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_slash_treasury_balance \
  --network $NETWORK \
  --source $ADMIN_KEY > $BACKUP_DIR/slash_treasury_$TIMESTAMP.json

echo "Backup completed: $BACKUP_DIR"
```

## Off-Chain Backup Strategy

### Database Backups

Maintain off-chain databases of critical contract state:

1. **Loan Records Database**
   - Store all loan records with timestamps
   - Track repayment history
   - Maintain default records

2. **Vouch Records Database**
   - Store all vouch records
   - Track stake changes
   - Maintain voucher history

3. **Admin Audit Log**
   - Store all admin actions
   - Track configuration changes
   - Maintain governance decisions

### Backup Schedule

- **Hourly**: Export contract balance and paused state
- **Daily**: Full export of all loan and vouch records
- **Weekly**: Full database backup with compression
- **Monthly**: Archive backups to cold storage

### Backup Storage

```
backups/
├── contract_state/
│   ├── config_*.json
│   ├── audit_log_*.json
│   └── slash_treasury_*.json
├── loan_records/
│   └── loans_*.json
├── vouch_records/
│   └── vouches_*.json
└── archives/
    └── backup_*.tar.gz
```

## Recovery Procedures

### Scenario 1: Contract Paused Unexpectedly

**Symptoms**: Contract is paused but no admin action was taken

**Recovery Steps**:
1. Verify contract status: `stellar contract invoke --id $CONTRACT_ID --fn get_paused`
2. Check admin audit log for unauthorized pause
3. If unauthorized, unpause immediately: `stellar contract invoke --id $CONTRACT_ID --fn unpause --source $ADMIN_KEY`
4. Investigate root cause in audit logs

### Scenario 2: Incorrect Configuration

**Symptoms**: Yield rate, slash rate, or other config is wrong

**Recovery Steps**:
1. Export current config: `stellar contract invoke --id $CONTRACT_ID --fn get_config`
2. Compare against backup config from before the change
3. Identify the incorrect parameter
4. Call `update_config` with correct values
5. Verify change: `stellar contract invoke --id $CONTRACT_ID --fn get_config`

### Scenario 3: Loan Record Corruption

**Symptoms**: Loan record is missing or has incorrect data

**Recovery Steps**:
1. Query loan from contract: `stellar contract invoke --id $CONTRACT_ID --fn get_loan --borrower $BORROWER`
2. Compare against backup loan records database
3. If contract record is corrupted:
   - Document the discrepancy
   - Contact affected borrower and vouchers
   - Manually reconcile using backup data
   - Update contract state if necessary (requires admin action)

### Scenario 4: Yield Reserve Depleted

**Symptoms**: Repayment fails with `InsufficientFunds` error

**Recovery Steps**:
1. Check contract balance: `stellar contract invoke --id $CONTRACT_ID --fn get_contract_balance`
2. Check pending repayments: Query all active loans
3. Calculate required yield: `sum(loan.total_yield for all active loans)`
4. If balance < required yield:
   - Transfer additional XLM to contract: `stellar contract invoke --id $CONTRACT_ID --fn transfer --to $CONTRACT_ID --amount $AMOUNT`
   - Verify balance increased
   - Retry repayment

### Scenario 5: Admin Key Compromise

**Symptoms**: Unauthorized admin actions in audit log

**Recovery Steps**:
1. Immediately pause contract: `stellar contract invoke --id $CONTRACT_ID --fn pause --source $SAFE_ADMIN_KEY`
2. Review audit log for unauthorized actions
3. Identify compromised admin key
4. Rotate admin keys:
   - Call `propose_admin` with new admin address
   - Call `accept_admin` from new admin account
   - Remove compromised admin from admins list
5. Unpause contract: `stellar contract invoke --id $CONTRACT_ID --fn unpause --source $SAFE_ADMIN_KEY`

## Disaster Recovery Runbook

### Full Contract Failure

If the contract becomes completely unusable:

1. **Assess Damage**
   - Determine what state is corrupted
   - Identify affected borrowers and vouchers
   - Calculate total exposure

2. **Pause Contract**
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --fn pause \
     --network $NETWORK \
     --source $ADMIN_KEY
   ```

3. **Prepare Upgrade**
   - Build fixed WASM
   - Validate upgrade: `stellar contract invoke --id $CONTRACT_ID --fn validate_upgrade --new_wasm_hash $HASH`
   - Test on testnet first

4. **Execute Upgrade**
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --fn upgrade \
     --network $NETWORK \
     --source $ADMIN_KEY \
     -- --new_wasm_hash $HASH
   ```

5. **Verify Recovery**
   - Check health: `stellar contract invoke --id $CONTRACT_ID --fn health_check`
   - Verify critical data: `stellar contract invoke --id $CONTRACT_ID --fn get_config`
   - Test basic operations on testnet

6. **Unpause and Communicate**
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --fn unpause \
     --network $NETWORK \
     --source $ADMIN_KEY
   ```
   - Notify all users of recovery
   - Provide status updates

### Data Loss Recovery

If contract data is lost:

1. **Restore from Backup**
   - Identify most recent valid backup
   - Extract loan and vouch records
   - Prepare migration script

2. **Recreate State**
   - For each loan record: Call `request_loan` with original parameters
   - For each vouch record: Call `vouch` with original parameters
   - Verify totals match backup

3. **Reconcile Repayments**
   - For each repaid loan: Call `repay` with original payment amount
   - Verify yield distribution matches backup
   - Check voucher balances

4. **Verify Integrity**
   - Compare restored state against backup
   - Check all balances match
   - Verify no data loss

## Monitoring and Alerts

### Health Check Monitoring

Regularly monitor contract health:

```bash
# Check health every 5 minutes
*/5 * * * * stellar contract invoke --id $CONTRACT_ID --fn health_check --network $NETWORK | jq '.is_healthy'
```

Alert if:
- `is_healthy` is false
- `initialized` is false
- `yield_reserve_solvent` is false
- Any issues are present

### Backup Verification

Verify backups are valid:

```bash
# Weekly backup verification
0 0 * * 0 ./verify-backups.sh
```

Verify:
- All backup files exist
- Files are not corrupted
- Data can be parsed
- Timestamps are recent

## Testing Recovery Procedures

### Monthly Disaster Recovery Drill

1. Restore from backup to testnet
2. Verify all data matches
3. Test critical operations
4. Document any issues
5. Update procedures as needed

### Backup Restoration Test

```bash
#!/bin/bash
# test-backup-restore.sh

# Restore latest backup
LATEST_BACKUP=$(ls -t backups/contract_state/config_*.json | head -1)
echo "Testing restore from: $LATEST_BACKUP"

# Parse and verify
jq . $LATEST_BACKUP > /dev/null || exit 1
echo "Backup is valid JSON"

# Check required fields
jq '.admins, .admin_threshold, .token' $LATEST_BACKUP > /dev/null || exit 1
echo "Backup contains required fields"

echo "Backup restoration test passed"
```

## Checklist

- [x] Automated backup script deployed (`scripts/backup.sh`)
- [ ] Backup storage configured
- [x] Backup verification script deployed (`scripts/test_backup_restore.sh`)
- [x] Weekly automated restore drill scheduled (`.github/workflows/backup-restore-test.yml`, Issue #1225)
- [ ] Disaster recovery runbook reviewed
- [ ] Team trained on recovery procedures
- [ ] Monthly disaster recovery drills scheduled
- [ ] Monitoring and alerts configured
- [ ] Off-chain database backups enabled
- [ ] Cold storage backups configured
- [x] Recovery procedures tested on testnet (via the weekly drill, when `BACKUP_TEST_STAGING_CONTRACT_ID` is configured)
