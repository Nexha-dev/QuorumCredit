#!/bin/bash
# test_backup_restore.sh — Automated backup/restore testing for QuorumCredit (Issue #1225).
#
# Runs backup.sh against the target network, restores the result into a
# disposable staging contract via restore.sh, and verifies the restored state
# is correct before recording a pass/fail record. Intended to run weekly (see
# .github/workflows/backup-restore-test.yml) so restore capability is proven
# on a schedule instead of only being discovered to be broken during a real
# incident.
#
# This script tests the MECHANICS of backup + restore (archive integrity,
# manifest completeness, scenario 6 replay), not disaster judgment calls
# (scenarios 1-5 in restore.sh remain operator-guided runbooks — see
# docs/backup-recovery-guide.md).
#
# Usage:
#   ./scripts/test_backup_restore.sh [--network <network>]
#
# Required environment variables (or .env entries):
#   CONTRACT_ID          — Contract to back up (typically a staging deployment)
#   ADMIN_KEY            — Secret key for read-only queries (S...)
#
# Optional environment variables:
#   NETWORK               — Stellar network (default: testnet)
#   STAGING_CONTRACT_ID   — Contract to restore INTO for the restore drill (default: same as CONTRACT_ID,
#                           must be set to a distinct disposable contract for an executed drill)
#   STAGING_ADMIN_KEY     — Admin key for the staging restore target (default: ADMIN_KEY)
#   BACKUP_TEST_HISTORY   — Path to the JSONL coverage/success-rate log (default: backups/test-results/history.jsonl)
#   BACKUP_TEST_EXECUTE   — "true" to actually --execute the restore against STAGING_CONTRACT_ID (default: false,
#                           i.e. dry-run only). Never execute against the source contract.
#   BACKUP_TEST_REQUIRE_RESTORE — Require a real executed restore (default: true).
#
# Exit code is non-zero if the drill fails, so this can gate CI / page on-call.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

ENV_FILE="$PROJECT_ROOT/.env"
if [ -f "$ENV_FILE" ]; then
    set -o allexport
    # shellcheck source=/dev/null
    source "$ENV_FILE"
    set +o allexport
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --network) NETWORK="${2:?'--network requires a value'}"; shift 2 ;;
        *) echo "Error: Unknown argument: $1" >&2; exit 1 ;;
    esac
done

NETWORK="${NETWORK:-testnet}"
STAGING_CONTRACT_ID="${STAGING_CONTRACT_ID:-}"
STAGING_ADMIN_KEY="${STAGING_ADMIN_KEY:-${ADMIN_KEY:-}}"
BACKUP_TEST_HISTORY="${BACKUP_TEST_HISTORY:-$PROJECT_ROOT/backups/test-results/history.jsonl}"
BACKUP_TEST_EXECUTE="${BACKUP_TEST_EXECUTE:-false}"
BACKUP_TEST_REQUIRE_RESTORE="${BACKUP_TEST_REQUIRE_RESTORE:-true}"

mkdir -p "$(dirname "$BACKUP_TEST_HISTORY")"

if ! command -v jq &>/dev/null; then
    echo "Error: 'jq' not found." >&2
    exit 1
fi

START_TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
START_EPOCH=$(date -u +%s)
RUN_ID="drill_$(date -u +%Y%m%d_%H%M%SZ)"
FAILURE_REASON=""
STATUS="pass"

record_preflight_failure() {
    local reason="$1"
    local duration_seconds
    duration_seconds=$(($(date -u +%s) - START_EPOCH))
    local result_json
    result_json=$(jq -n \
        --arg run_id "$RUN_ID" \
        --arg timestamp "$START_TS" \
        --arg network "$NETWORK" \
        --arg contract_id "${CONTRACT_ID:-}" \
        --arg staging_contract_id "$STAGING_CONTRACT_ID" \
        --arg failure_reason "$reason" \
        --argjson duration_seconds "$duration_seconds" \
        '{run_id: $run_id, timestamp: $timestamp, network: $network, contract_id: $contract_id,
          staging_contract_id: $staging_contract_id, status: "fail", failure_reason: $failure_reason,
          completeness: "unknown", checksum_ok: "false", restore_ok: "false",
          correctness: "skipped", duration_seconds: $duration_seconds}')
    printf '%s\n' "$result_json" >> "$BACKUP_TEST_HISTORY"
    echo "::error::$reason" >&2
    echo "Backup verification preflight failed: $reason" >&2
    exit 1
}

for var in CONTRACT_ID ADMIN_KEY; do
    if [ -z "${!var:-}" ]; then
        record_preflight_failure "$var is not set"
    fi
done

if [ "$BACKUP_TEST_REQUIRE_RESTORE" = "true" ] && [ "$BACKUP_TEST_EXECUTE" != "true" ]; then
    record_preflight_failure "BACKUP_TEST_EXECUTE=true is required for an automated restore drill"
fi

if [ "$BACKUP_TEST_EXECUTE" = "true" ]; then
    if [ -z "$STAGING_CONTRACT_ID" ]; then
        record_preflight_failure "STAGING_CONTRACT_ID is required for an executed restore"
    fi
    if [ "$STAGING_CONTRACT_ID" = "$CONTRACT_ID" ]; then
        record_preflight_failure "STAGING_CONTRACT_ID must differ from CONTRACT_ID; refusing to restore into the source contract"
    fi
fi

echo "QuorumCredit backup/restore test drill — $RUN_ID"
echo "  Network            : $NETWORK"
echo "  Source contract     : $CONTRACT_ID"
echo "  Staging contract    : ${STAGING_CONTRACT_ID:-<none>}"
echo "  Execute restore     : $BACKUP_TEST_EXECUTE"
echo ""

# ── Step 1: Take a fresh backup ────────────────────────────────────────────────

echo "Step 1/4 — Running backup.sh..."
BACKUP_LOG=$(mktemp)
if CONTRACT_ID="$CONTRACT_ID" ADMIN_KEY="$ADMIN_KEY" NETWORK="$NETWORK" \
    "$SCRIPT_DIR/backup.sh" > "$BACKUP_LOG" 2>&1; then
    echo "  [OK] backup.sh completed"
else
    STATUS="fail"
    FAILURE_REASON="backup.sh exited non-zero"
    echo "  [ERR] backup.sh failed — see below" >&2
    cat "$BACKUP_LOG" >&2
fi

LATEST_BACKUP_DIR=$(find "$PROJECT_ROOT/backups" -maxdepth 1 -mindepth 1 -type d -name '2*' | sort | tail -1)
LATEST_ARCHIVE=$(find "$PROJECT_ROOT/backups" -maxdepth 1 -name 'backup_*.tar.gz' | sort | tail -1)

# ── Step 2: Verify manifest completeness + checksum ────────────────────────────

COMPLETE="unknown"
CHECKSUM_OK="false"
if [ "$STATUS" = "pass" ] && [ -n "$LATEST_BACKUP_DIR" ] && [ -f "$LATEST_BACKUP_DIR/manifest.json" ]; then
    echo ""
    echo "Step 2/4 — Verifying manifest completeness and checksum..."
    MANIFEST="$LATEST_BACKUP_DIR/manifest.json"
    COMPLETE=$(jq -r '.completeness.complete' "$MANIFEST")
    RECORDED_CHECKSUM=$(jq -r '.checksum' "$MANIFEST" | sed 's/^sha256://')
    RECOMPUTED_CHECKSUM=$(find "$LATEST_BACKUP_DIR" -type f ! -name "manifest.json" | sort | xargs sha256sum | sha256sum | cut -d' ' -f1)

    if [ "$RECORDED_CHECKSUM" = "$RECOMPUTED_CHECKSUM" ]; then
        CHECKSUM_OK="true"
        echo "  [OK] checksum matches recomputed value"
    else
        CHECKSUM_OK="false"
        STATUS="fail"
        FAILURE_REASON="manifest checksum mismatch (backup corrupted or truncated in transit)"
        echo "  [ERR] checksum mismatch: manifest=$RECORDED_CHECKSUM recomputed=$RECOMPUTED_CHECKSUM" >&2
    fi

    if [ "$COMPLETE" = "false" ]; then
        STATUS="fail"
        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }backup completeness proof reported incomplete address coverage"
        echo "  [ERR] backup reports incomplete address coverage (see completeness.missing_estimate)" >&2
    else
        echo "  [OK] completeness = $COMPLETE"
    fi

    # Required top-level snapshot files a restorable backup must contain — the
    # minimum an operator needs for scenarios 1-4 in restore.sh.
    for required_file in config.json admins.json paused.json health.json; do
        if [ ! -f "$LATEST_BACKUP_DIR/$required_file" ]; then
            STATUS="fail"
            FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }missing required snapshot file: $required_file"
            echo "  [ERR] required snapshot file missing: $required_file" >&2
        fi
    done
else
    if [ "$STATUS" = "pass" ]; then
        STATUS="fail"
        FAILURE_REASON="no manifest.json produced by backup.sh"
    fi
fi

# ── Step 3: Restore drill against staging ──────────────────────────────────────

RESTORE_LOG=$(mktemp)
RESTORE_OK="skipped"
if [ "$STATUS" = "pass" ] && [ -n "$STAGING_CONTRACT_ID" ] && [ -n "$LATEST_ARCHIVE" ]; then
    echo ""
    echo "Step 3/4 — Restoring into staging ($STAGING_CONTRACT_ID)..."
    RESTORE_ARGS=(--backup "$LATEST_ARCHIVE" --network "$NETWORK" --scenario 6)
    if [ "$BACKUP_TEST_EXECUTE" = "true" ]; then
        RESTORE_ARGS+=(--execute)
    fi

    if CONTRACT_ID="$STAGING_CONTRACT_ID" ADMIN_KEY="${STAGING_ADMIN_KEY:-$ADMIN_KEY}" \
        RESTORE_STATE_FILE="$LATEST_BACKUP_DIR/.restore-test.state.json" \
        "$SCRIPT_DIR/restore.sh" "${RESTORE_ARGS[@]}" > "$RESTORE_LOG" 2>&1; then
        RESTORE_OK="true"
        echo "  [OK] restore.sh scenario 6 completed against staging"
    else
        RESTORE_OK="false"
        STATUS="fail"
        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }restore.sh scenario 6 failed against staging"
        echo "  [ERR] restore.sh failed — see below" >&2
        cat "$RESTORE_LOG" >&2
    fi
else
    echo ""
    echo "Step 3/4 — Skipped (no STAGING_CONTRACT_ID configured, or backup failed upstream)."
fi

# ── Step 4: Verify restored state correctness ─────────────────────────────────
#
# Re-run a subset of read-only queries against the staging contract after the
# restore drill and diff them against the backed-up values. This is what
# actually proves restore capability — a restore that "completes" but leaves
# staging state diverging from the backup would otherwise look like a pass.

CORRECTNESS="skipped"
if [ "$STATUS" = "pass" ] && [ "$RESTORE_OK" = "true" ] && [ "$BACKUP_TEST_EXECUTE" = "true" ]; then
    echo ""
    echo "Step 4/4 — Verifying restored state correctness..."
    
    CORRECTNESS_CHECKS_PASSED=0
    CORRECTNESS_CHECKS_FAILED=0
    
    if command -v stellar &>/dev/null; then
        # Check 1: Paused state
        BACKUP_PAUSED=$(jq -c . "$LATEST_BACKUP_DIR/paused.json" 2>/dev/null || echo "null")
        STAGING_PAUSED=$(stellar contract invoke --id "$STAGING_CONTRACT_ID" \
            --source "${STAGING_ADMIN_KEY:-$ADMIN_KEY}" --network "$NETWORK" \
            -- get_paused 2>/dev/null || echo "null")
        if [ "$BACKUP_PAUSED" = "$STAGING_PAUSED" ]; then
            CORRECTNESS_CHECKS_PASSED=$((CORRECTNESS_CHECKS_PASSED + 1))
            echo "  [OK] restored paused-state matches backup"
        else
            CORRECTNESS_CHECKS_FAILED=$((CORRECTNESS_CHECKS_FAILED + 1))
            STATUS="fail"
            FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }restored state diverges from backup (get_paused mismatch)"
            echo "  [ERR] restored state mismatch: backup=$BACKUP_PAUSED staging=$STAGING_PAUSED" >&2
        fi
        
        # Check 2-5: Loan and vouch records for sampled borrowers
        # Determine which borrowers to sample: use env var BACKUP_TEST_SAMPLE_BORROWERS if set,
        # otherwise sample all borrowers from the backed-up derived_addresses.txt
        SAMPLE_BORROWERS_FILE="${BACKUP_TEST_SAMPLE_BORROWERS:-$LATEST_BACKUP_DIR/derived_addresses.txt}"
        
        if [ -f "$SAMPLE_BORROWERS_FILE" ]; then
            SAMPLE_COUNT=0
            LOANS_DIR="$LATEST_BACKUP_DIR/loans"
            VOUCHES_DIR="$LATEST_BACKUP_DIR/vouches"
            
            while IFS= read -r borrower; do
                [ -z "$borrower" ] && continue
                SAMPLE_COUNT=$((SAMPLE_COUNT + 1))
                
                safe_name=$(echo -n "$borrower" | sha256sum | cut -c1-16)
                
                # Check loan record
                if [ -f "$LOANS_DIR/${safe_name}.json" ]; then
                    BACKUP_LOAN=$(jq -c . "$LOANS_DIR/${safe_name}.json" 2>/dev/null || echo "null")
                    STAGING_LOAN=$(stellar contract invoke --id "$STAGING_CONTRACT_ID" \
                        --source "${STAGING_ADMIN_KEY:-$ADMIN_KEY}" --network "$NETWORK" \
                        -- get_loan --borrower "$borrower" 2>/dev/null | jq -c . || echo "null")
                    
                    if [ "$BACKUP_LOAN" = "$STAGING_LOAN" ]; then
                        CORRECTNESS_CHECKS_PASSED=$((CORRECTNESS_CHECKS_PASSED + 1))
                    else
                        CORRECTNESS_CHECKS_FAILED=$((CORRECTNESS_CHECKS_FAILED + 1))
                        STATUS="fail"
                        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }loan record mismatch for borrower $borrower"
                        echo "  [ERR] loan record mismatch for $borrower" >&2
                        echo "        backup: $BACKUP_LOAN" >&2
                        echo "        staging: $STAGING_LOAN" >&2
                    fi
                fi
                
                # Check vouch records
                if [ -f "$VOUCHES_DIR/${safe_name}.json" ]; then
                    BACKUP_VOUCHES=$(jq -c . "$VOUCHES_DIR/${safe_name}.json" 2>/dev/null || echo "null")
                    STAGING_VOUCHES=$(stellar contract invoke --id "$STAGING_CONTRACT_ID" \
                        --source "${STAGING_ADMIN_KEY:-$ADMIN_KEY}" --network "$NETWORK" \
                        -- get_vouches --borrower "$borrower" 2>/dev/null | jq -c . || echo "null")
                    
                    if [ "$BACKUP_VOUCHES" = "$STAGING_VOUCHES" ]; then
                        CORRECTNESS_CHECKS_PASSED=$((CORRECTNESS_CHECKS_PASSED + 1))
                    else
                        CORRECTNESS_CHECKS_FAILED=$((CORRECTNESS_CHECKS_FAILED + 1))
                        STATUS="fail"
                        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }vouch records mismatch for borrower $borrower"
                        echo "  [ERR] vouch records mismatch for $borrower" >&2
                        echo "        backup: $BACKUP_VOUCHES" >&2
                        echo "        staging: $STAGING_VOUCHES" >&2
                    fi
                fi
                
                # Check loan status
                if [ -f "$LOANS_DIR/${safe_name}_status.json" ]; then
                    BACKUP_STATUS=$(jq -c . "$LOANS_DIR/${safe_name}_status.json" 2>/dev/null || echo "null")
                    STAGING_STATUS=$(stellar contract invoke --id "$STAGING_CONTRACT_ID" \
                        --source "${STAGING_ADMIN_KEY:-$ADMIN_KEY}" --network "$NETWORK" \
                        -- loan_status --borrower "$borrower" 2>/dev/null | jq -c . || echo "null")
                    
                    if [ "$BACKUP_STATUS" = "$STAGING_STATUS" ]; then
                        CORRECTNESS_CHECKS_PASSED=$((CORRECTNESS_CHECKS_PASSED + 1))
                    else
                        CORRECTNESS_CHECKS_FAILED=$((CORRECTNESS_CHECKS_FAILED + 1))
                        STATUS="fail"
                        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }loan_status mismatch for borrower $borrower"
                        echo "  [ERR] loan_status mismatch for $borrower" >&2
                        echo "        backup: $BACKUP_STATUS" >&2
                        echo "        staging: $STAGING_STATUS" >&2
                    fi
                fi
                
                # Check total vouched amount
                if [ -f "$VOUCHES_DIR/${safe_name}_total.json" ]; then
                    BACKUP_TOTAL=$(jq -c . "$VOUCHES_DIR/${safe_name}_total.json" 2>/dev/null || echo "null")
                    STAGING_TOTAL=$(stellar contract invoke --id "$STAGING_CONTRACT_ID" \
                        --source "${STAGING_ADMIN_KEY:-$ADMIN_KEY}" --network "$NETWORK" \
                        -- total_vouched --borrower "$borrower" 2>/dev/null | jq -c . || echo "null")
                    
                    if [ "$BACKUP_TOTAL" = "$STAGING_TOTAL" ]; then
                        CORRECTNESS_CHECKS_PASSED=$((CORRECTNESS_CHECKS_PASSED + 1))
                    else
                        CORRECTNESS_CHECKS_FAILED=$((CORRECTNESS_CHECKS_FAILED + 1))
                        STATUS="fail"
                        FAILURE_REASON="${FAILURE_REASON:+$FAILURE_REASON; }total_vouched mismatch for borrower $borrower"
                        echo "  [ERR] total_vouched mismatch for $borrower" >&2
                        echo "        backup: $BACKUP_TOTAL" >&2
                        echo "        staging: $STAGING_TOTAL" >&2
                    fi
                fi
            done < "$SAMPLE_BORROWERS_FILE"
            
            echo "  Sampled $SAMPLE_COUNT borrower(s): $CORRECTNESS_CHECKS_PASSED checks passed, $CORRECTNESS_CHECKS_FAILED failed"
        fi
        
        # Determine overall correctness
        if [ "$CORRECTNESS_CHECKS_FAILED" -eq 0 ] && [ "$CORRECTNESS_CHECKS_PASSED" -gt 0 ]; then
            CORRECTNESS="true"
        elif [ "$CORRECTNESS_CHECKS_PASSED" -eq 0 ] && [ "$CORRECTNESS_CHECKS_FAILED" -eq 0 ]; then
            CORRECTNESS="skipped"
            echo "  [WARN] No correctness checks were performed — no borrowers in backup to sample" >&2
        else
            CORRECTNESS="false"
        fi
    fi
else
    echo ""
    echo "Step 4/4 — Skipped (dry-run mode or no successful execute-restore to verify)."
fi

END_EPOCH=$(date -u +%s)
DURATION_SECONDS=$((END_EPOCH - START_EPOCH))

# ── Record result to history for coverage/success-rate tracking ──────────────

RESULT_JSON=$(jq -n \
    --arg run_id "$RUN_ID" \
    --arg timestamp "$START_TS" \
    --arg network "$NETWORK" \
    --arg contract_id "$CONTRACT_ID" \
    --arg staging_contract_id "${STAGING_CONTRACT_ID:-}" \
    --arg status "$STATUS" \
    --arg failure_reason "$FAILURE_REASON" \
    --arg completeness "$COMPLETE" \
    --arg checksum_ok "$CHECKSUM_OK" \
    --arg restore_ok "$RESTORE_OK" \
    --arg correctness "$CORRECTNESS" \
    --argjson duration_seconds "$DURATION_SECONDS" \
    '{run_id: $run_id, timestamp: $timestamp, network: $network, contract_id: $contract_id,
      staging_contract_id: $staging_contract_id, status: $status, failure_reason: $failure_reason,
      completeness: $completeness, checksum_ok: $checksum_ok, restore_ok: $restore_ok,
      correctness: $correctness, duration_seconds: $duration_seconds}')

echo "$RESULT_JSON" >> "$BACKUP_TEST_HISTORY"

# ── Coverage / success-rate summary over recorded history ────────────────────

TOTAL_RUNS=$(wc -l < "$BACKUP_TEST_HISTORY" | tr -d ' ')
PASS_RUNS=$(grep -c '"status":"pass"' "$BACKUP_TEST_HISTORY" || true)
SUCCESS_RATE=$(awk -v p="$PASS_RUNS" -v t="$TOTAL_RUNS" 'BEGIN { printf "%.1f", (t > 0 ? (p / t * 100) : 0) }')

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "Drill result: $STATUS"
[ -n "$FAILURE_REASON" ] && echo "  Reason: $FAILURE_REASON"
echo "  Duration: ${DURATION_SECONDS}s"
echo "  History : $BACKUP_TEST_HISTORY"
echo "  Coverage: $PASS_RUNS/$TOTAL_RUNS runs passed cumulatively ($SUCCESS_RATE% success rate)"
echo "═══════════════════════════════════════════════════════════"

rm -f "$BACKUP_LOG" "$RESTORE_LOG"

if [ "$STATUS" != "pass" ]; then
    exit 1
fi
