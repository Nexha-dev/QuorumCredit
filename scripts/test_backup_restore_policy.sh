#!/usr/bin/env bash
# Regression checks for fail-closed automated restore policy.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

export CONTRACT_ID="CSOURCE123"
export ADMIN_KEY="SSECRET123"
export STAGING_CONTRACT_ID="CSTAGING456"
export BACKUP_TEST_REQUIRE_RESTORE="true"

assert_preflight_failure() {
    local history_file="$1"
    local expected_reason="$2"
    shift 2

    if env "BACKUP_TEST_HISTORY=$history_file" "$@" "$SCRIPT_DIR/test_backup_restore.sh" >/dev/null 2>&1; then
        echo "Expected backup restore preflight to fail: $expected_reason" >&2
        exit 1
    fi

    local result
    result=$(jq -s '.[-1]' "$history_file")
    [[ $(jq -r '.status' <<< "$result") == "fail" ]]
    [[ $(jq -r '.restore_ok' <<< "$result") == "false" ]]
    [[ $(jq -r '.failure_reason' <<< "$result") == *"$expected_reason"* ]]
}

assert_preflight_failure \
    "$TEMP_DIR/dry-run-history.jsonl" \
    "BACKUP_TEST_EXECUTE=true" \
    env BACKUP_TEST_EXECUTE=false

assert_preflight_failure \
    "$TEMP_DIR/source-target-history.jsonl" \
    "must differ from CONTRACT_ID" \
    env BACKUP_TEST_EXECUTE=true STAGING_CONTRACT_ID="$CONTRACT_ID"

echo "Backup restore safety policy tests passed."