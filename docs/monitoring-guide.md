# Monitoring and Alerting Setup Guide

> **Role:** Quick start. This is the short, opinionated path to get Prometheus, Grafana, and AlertManager scraping the QuorumCredit indexer's `/metrics` endpoint.
>
> For the complete reference — full metric semantics, every dashboard panel, the full alert catalog, and detailed runbooks — see [monitoring-setup-guide.md](./monitoring-setup-guide.md).
>
> **Keep both in sync:** if you change a metric, alert, or dashboard here, update the complete reference too (and vice versa). See the checklist in [CONTRIBUTING.md](../CONTRIBUTING.md).

Comprehensive monitoring for QuorumCredit protocol operations.

## Prerequisites

This guide assumes the **QuorumCredit indexer** (`tools/indexer/`) is deployed and serving Prometheus metrics at `/metrics`. The indexer derives all metrics from actual on-chain events — no fabricated contract-state calls.

See [tools/indexer/README.md](../tools/indexer/src/main.rs) or `cargo run -p quorum-credit-indexer -- --help` for deployment instructions.

## Prometheus Configuration

### Scrape the indexer's `/metrics` endpoint

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'quorum-credit-indexer'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
```

### Available Metrics

The indexer exposes the following metrics sourced entirely from the Soroban event stream — no `get_contract_data` calls:

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `qc_indexer_ledger_height` | Gauge | — | Last processed ledger sequence |
| `qc_indexer_events_total` | Counter | `category`, `action` | Events indexed |
| `qc_indexer_gap_detected_total` | Counter | — | Retention-window gaps detected |
| `qc_indexer_reorgs_detected_total` | Counter | — | Ledger reorgs detected |
| `qc_indexer_errors_total` | Counter | `error_code` | Indexer-level errors |
| `qc_indexer_backfill_events_total` | Counter | — | Events indexed during backfill |
| `qc_loan_volume_total` | Counter | `token` | Total stroops loaned |
| `qc_loan_count_total` | Counter | — | Total loans created |
| `qc_active_loans` | Gauge | — | Currently active (unrepaid) loans |
| `qc_slash_events_total` | Counter | — | Total slash events |
| `qc_slash_amount_total` | Counter | `token` | Total stroops slashed |
| `qc_vouch_count` | Gauge | — | Currently active vouches |
| `qc_attestor_active_vouches` | Gauge | `attestor` | Active borrower vouches per observed attestor |
| `qc_attestor_active_stake_stroops` | Gauge | `attestor` | Active vouch stake per observed attestor, in stroops |
| `qc_attestor_last_vouch_ledger` | Gauge | `attestor` | Ledger of the latest indexed vouch event for an attestor |
| `qc_attestor_vouch_events_total` | Counter | `attestor`, `action` | Indexed create/increase/decrease/withdraw events per attestor |
| `qc_ws_queue_drops_total` | Counter | `type` | WebSocket queue overflow drops (`loan` or `metrics`) |

The **Attestor Health** Grafana dashboard visualizes event-derived stake, active vouches, event rates, and ledgers since last vouch activity. These metrics describe on-chain vouch activity; they do not indicate attestor process uptime or off-chain signing availability. `AttestorNoActiveStake` warns when an observed attestor has no active stake for five minutes, so operators can verify that a withdrawal or rotation was expected.

### Metric Semantics

- **Counters** (`_total` suffix) are monotonic — they persist across indexer restarts via rebuild from the event store.
- **Gauges** (`qc_active_loans`, `qc_vouch_count`, `qc_indexer_ledger_height`) are set from the event stream and reset on restart.
- **Labels** (`token`, `category`, `action`) are fixed at metric creation time. The indexer records events with the appropriate label combinations as they arrive.

## Grafana Dashboards

### Dashboard 1: Protocol Overview

```json
{
  "dashboard": {
    "title": "QuorumCredit Protocol Overview",
    "panels": [
      {
        "title": "Active Loans",
        "targets": [
          {
            "expr": "qc_active_loans"
          }
        ]
      },
      {
        "title": "Total Loan Volume (XLM)",
        "targets": [
          {
            "expr": "qc_loan_volume_total / 10000000"
          }
        ]
      },
      {
        "title": "Active Vouches",
        "targets": [
          {
            "expr": "qc_vouch_count"
          }
        ]
      },
      {
        "title": "Indexer Ledger Height",
        "targets": [
          {
            "expr": "qc_indexer_ledger_height"
          }
        ]
      }
    ]
  }
}
```

### Dashboard 2: Risk Metrics

```json
{
  "dashboard": {
    "title": "QuorumCredit Risk Metrics",
    "panels": [
      {
        "title": "Slash Events (24h)",
        "targets": [
          {
            "expr": "increase(qc_slash_events_total[24h])"
          }
        ]
      },
      {
        "title": "Total Amount Slashed (XLM)",
        "targets": [
          {
            "expr": "qc_slash_amount_total / 10000000"
          }
        ]
      },
      {
        "title": "Error Rate (5m)",
        "targets": [
          {
            "expr": "rate(qc_indexer_errors_total[5m])"
          }
        ]
      }
    ]
  }
}
```

### Dashboard 3: Indexer Health

```json
{
  "dashboard": {
    "title": "Indexer Health",
    "panels": [
      {
        "title": "Events per Minute",
        "targets": [
          {
            "expr": "rate(qc_indexer_events_total[1m])"
          }
        ]
      },
      {
        "title": "Reorgs Detected",
        "targets": [
          {
            "expr": "rate(qc_indexer_reorgs_detected_total[1h])"
          }
        ]
      },
      {
        "title": "Backfill Events",
        "targets": [
          {
            "expr": "rate(qc_indexer_backfill_events_total[1h])"
          }
        ]
      }
    ]
  }
}
```

## Alerting Rules

### Alert Rules (Prometheus)

```yaml
# alerts.yml
groups:
  - name: quorum_credit
    interval: 30s
    rules:
      # Indexer down
      - alert: IndexerDown
        expr: up{job="quorum-credit-indexer"} == 0
        for: 1m
        annotations:
          summary: "QuorumCredit indexer is down"
          description: "No metrics received for 1 minute"

      # Ledger height stalled
      - alert: IndexerStalled
        expr: qc_indexer_ledger_height == 0
        for: 5m
        annotations:
          summary: "Indexer has not processed any ledgers"
          description: "Ledger height is 0 for 5 minutes"

      # Indexer errors
      - alert: IndexerErrors
        expr: rate(qc_indexer_errors_total[5m]) > 0.1
        for: 5m
        annotations:
          summary: "Indexer error rate elevated"
          description: "Error rate: {{ $value | humanizePercentage }}"

      # Reorg detected
      - alert: LedgerReorgDetected
        expr: increase(qc_indexer_reorgs_detected_total[5m]) > 0
        for: 1m
        annotations:
          summary: "Soroban ledger reorg detected"
          description: "Indexer rolled back and re-indexed affected events"

      # Excessive slashing
      - alert: ExcessiveSlashing
        expr: increase(qc_slash_events_total[1h]) > 10
        for: 5m
        annotations:
          summary: "Excessive slash events in 1 hour"
          description: "Slash events: {{ $value }}"

      # High active loan ratio vs total
      - alert: HighLoanUtilization
        expr: qc_active_loans > (qc_loan_count_total - qc_active_loans) * 5
        for: 5m
        annotations:
          summary: "Unusually high active-to-repaid loan ratio"
          description: "Active: {{ $value }} loans"
```

## Runbook for Common Alerts

### Alert: IndexerDown / IndexerStalled

**Severity:** Critical

**Symptoms:**
- No metrics from the indexer
- Dashboard data frozen

**Diagnosis:**
```bash
# Check process
systemctl status quorum-credit-indexer

# Check logs
journalctl -u quorum-credit-indexer --since "5 min ago"

# Check database
du -sh /data/indexer.db
sqlite3 /data/indexer.db "SELECT value FROM cursor WHERE key = 'last_ledger';"
```

**Resolution:**
1. Restart the indexer: `systemctl restart quorum-credit-indexer`
2. If the database is corrupted, restore from backup
3. If the RPC endpoint is down, check network connectivity

### Alert: IndexerErrors

**Severity:** High

**Symptoms:**
- Error rate > 10%

**Diagnosis:**
```bash
# Check error distribution
curl 'http://localhost:9090/metrics' | grep qc_indexer_errors_total

# Check indexer logs
journalctl -u quorum-credit-indexer --since "10 min ago" | grep ERROR
```

**Resolution:**
1. Check RPC endpoint health: `curl <rpc-url>/health`
2. Verify network connectivity
3. If persistent, consider rotating RPC endpoints

### Alert: LedgerReorgDetected

**Severity:** Medium

**Symptoms:**
- Spike in `qc_indexer_reorgs_detected_total`

**Diagnosis:**
```bash
# Query reorg audit log
sqlite3 /data/indexer.db "SELECT * FROM reorg_audit ORDER BY id DESC LIMIT 5;"
```

**Resolution:**
This is informational — the indexer automatically recovers. If reorgs are frequent, the Soroban network may be experiencing instability.

### Alert: ExcessiveSlashing

**Severity:** Medium

**Symptoms:**
- > 10 slash events in 1 hour

**Diagnosis:**
```bash
# Query recent slash events from the event store
sqlite3 /data/indexer.db "SELECT ledger, value_json FROM events WHERE category = 'loan' AND action = 'slash' ORDER BY ledger DESC LIMIT 20;"
```

**Resolution:**
1. Investigate borrower defaults
2. Check for coordinated attacks
3. Review voucher selection process
4. Consider adjusting slash threshold if legitimate

### Alert: HighLoanUtilization

**Severity:** Medium

**Symptoms:**
- Active loans >> repaid loans

**Diagnosis:**
```bash
# Check active vs total loan counts
curl 'http://localhost:9090/metrics' | grep -E 'qc_active_loans|qc_loan_count_total'
```

**Resolution:**
1. Check if borrowers are defaulting
2. Review repayment rates
3. Consider pausing new loans until existing ones are repaid

### Alert: InsuranceFundLowBalance

**Severity:** High

**Introduced:** Issue #1436

**Symptoms:**
- The contract emitted an `insurance_fund` / `low_balance` event
  (`qc_insurance_fund_low_balance_total` increased).
- A `claim_insurance_for_shortfall` call pushed the fund balance below the
  configured `insurance_fund_low_bal_thresh`.

**Why it matters:**
`claim_insurance_for_shortfall` only returns `InsurancePoolEmpty` once the fund
hits *exactly zero*. The low-balance event is the earlier warning: once the fund
is depleted, tail-risk voucher losses are absorbed by vouchers directly (see
[loss-waterfall.md](loss-waterfall.md)).

**Diagnosis:**
```bash
# Current fund balance vs. the configured threshold
curl 'http://localhost:9090/metrics' | grep -E 'qc_insurance_fund_balance'
# Recent low_balance events from the event store
sqlite3 /data/indexer.db "SELECT ledger, value_json FROM events WHERE category = 'insurance' AND action = 'low_balance' ORDER BY ledger DESC LIMIT 20;"
# Recent shortfall claims that drained the fund
sqlite3 /data/indexer.db "SELECT ledger, value_json FROM events WHERE category = 'insurance' AND action = 'contrib' ORDER BY ledger DESC LIMIT 20;"
```

**Resolution:**
1. Confirm the depletion is driven by legitimate defaults, not an attack
   (cross-check with `ExcessiveSlashing`).
2. Have admins top up the fund via `contribute_to_insurance_fund` (bounded by
   `insurance_fund_max_contrib`; the contribution emits an
   `insurance_fund` / `contrib` audit event).
3. If defaults are systemic, consider raising `insurance_fund_premium_bps` so the
   fund refills faster from organic fee accrual, and review the circuit breaker
   threshold.
4. Re-evaluate `insurance_fund_low_bal_thresh` if it is firing too early
   or too late relative to typical claim sizes.

## Monitoring Setup Checklist

- [ ] Prometheus installed and configured
- [ ] QuorumCredit indexer deployed and scraping
- [ ] Grafana dashboards imported
- [ ] Alert rules configured
- [ ] Alert channels (Slack, PagerDuty) configured
- [ ] On-call rotation established
- [ ] Runbooks documented and accessible
- [ ] Monitoring tested with synthetic transactions
- [ ] Dashboards accessible to ops team
- [ ] Metrics retention policy set (30 days minimum)

## References

Test protocol health with periodic transactions:

```python
import schedule
import time
from stellar_sdk import Keypair

def synthetic_test():
    """Run synthetic vouch -> loan -> repay cycle"""
    try:
        # Create test accounts
        voucher = Keypair.random()
        borrower = Keypair.random()
        
        # Fund accounts (testnet only)
        # ...
        
        # Vouch
        vouch(CONTRACT_ID, voucher, borrower.public_key, 100 * 10_000_000, TOKEN_ADDRESS)
        
        # Request loan
        request_loan(CONTRACT_ID, borrower, 50 * 10_000_000, 100 * 10_000_000, "Test", TOKEN_ADDRESS)
        
        # Repay
        repay(CONTRACT_ID, borrower, 51 * 10_000_000)
        
        print("Synthetic test passed")
    except Exception as e:
        print(f"Synthetic test failed: {e}")

schedule.every(1).hours.do(synthetic_test)

while True:
    schedule.run_pending()
    time.sleep(60)
```

---

## Health Endpoint (Issue #112)

The `health_check()` function in `QuorumCredit/src/health.rs` provides a structured view of contract operational status. Poll it from your monitoring stack to detect degraded conditions before they become outages.

### Invocation

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn health_check \
  --network testnet \
  --source $READ_ONLY_KEY
```

### Response Shape

```json
{
  "overall_status": "Healthy",
  "is_healthy": true,
  "initialized": true,
  "paused": false,
  "yield_reserve_solvent": true,
  "pubsub_connected": true,
  "revocation_store_connected": true,
  "webhook_registry_connected": true,
  "issues": []
}
```

### Status Levels

| `overall_status` | `is_healthy` | Meaning | Action |
|---|---|---|---|
| `Healthy` | `true` | All checks pass. Contract fully operational. | None |
| `Degraded` | `false` | Non-critical sub-system(s) unavailable; local fallbacks serving. | Investigate sub-system connectivity. Non-urgent. |
| `Down` | `false` | Critical check(s) failed. Contract cannot operate correctly. | **Page on-call immediately.** |

### Checks Performed

| Check | Field | Critical? | Down Condition |
|-------|-------|-----------|----------------|
| Contract initialized | `initialized` | ✅ Yes | `DataKey::Config` missing from storage |
| Yield reserve solvent | `yield_reserve_solvent` | ✅ Yes | Token balance < 10,000,000 stroops (1 XLM) |
| Contract paused | `paused` | ❌ No | Paused = `true` (informational; does not affect `overall_status`) |
| PubSub bus connected | `pubsub_connected` | ❌ No | Sentinel `DataKey::PubSubHealthy` is `false` or absent |
| RevocationStore connected | `revocation_store_connected` | ❌ No | Sentinel `DataKey::RevocationStoreHealthy` is `false` or absent |
| WebhookRegistry connected | `webhook_registry_connected` | ❌ No | Sentinel `DataKey::WebhookRegistryHealthy` is `false` or absent |

> [!NOTE]
> `paused` is reported as a field but does **not** by itself change `overall_status` to `Down`. The contract being paused is an intentional admin action, not a failure condition. Monitor `paused` separately if you want to alert on long pause windows.

### Degraded Status

When any non-critical sub-system sentinel is `false` or absent (never set), `overall_status` transitions from `Healthy` to `Degraded`. This indicates that:

- Redis-backed components (PubSub, RevocationStore, WebhookRegistry) are unavailable **but**
- The core contract is still processing loans, vouches, and repayments normally using local fallbacks.

A `Degraded` state should generate a **warning-severity alert**, not a page.

### Sub-System Sentinels

Off-chain relayer processes write boolean sentinels into contract instance storage via a restricted admin call. The health check reads these sentinels to determine sub-system connectivity.

```bash
# Example: relayer marks PubSub as healthy
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn set_subsystem_health \
  --network testnet \
  --source $RELAYER_KEY \
  -- \
  --admin_signers '["'$ADMIN_ADDRESS'"]' \
  --key PubSub \
  --healthy true
```

Keys and their defaults:

| Key                    | DataKey                     | Default (absent) |
|------------------------|-----------------------------|------------------|
| PubSub bus             | `DataKey::PubSubHealthy`    | `false` → Degraded |
| RevocationStore        | `DataKey::RevocationStoreHealthy` | `false` → Degraded |
| WebhookRegistry        | `DataKey::WebhookRegistryHealthy` | `false` → Degraded |

### Example Responses by Status

**Healthy — all systems operational:**
```json
{
  "overall_status": "Healthy",
  "is_healthy": true,
  "initialized": true,
  "paused": false,
  "yield_reserve_solvent": true,
  "pubsub_connected": true,
  "revocation_store_connected": true,
  "webhook_registry_connected": true,
  "issues": []
}
```

**Degraded — Redis backing services unreachable, contract serving normally:**
```json
{
  "overall_status": "Degraded",
  "is_healthy": false,
  "initialized": true,
  "paused": false,
  "yield_reserve_solvent": true,
  "pubsub_connected": false,
  "revocation_store_connected": false,
  "webhook_registry_connected": true,
  "issues": [
    "PubSub bus unreachable or sentinel not set — local event fallback active",
    "RevocationStore unreachable or sentinel not set — local cache fallback active"
  ]
}
```

**Down — contract not initialized or yield reserve depleted:**
```json
{
  "overall_status": "Down",
  "is_healthy": false,
  "initialized": true,
  "paused": false,
  "yield_reserve_solvent": false,
  "pubsub_connected": false,
  "revocation_store_connected": false,
  "webhook_registry_connected": false,
  "issues": [
    "Yield reserve below minimum threshold (1 XLM)"
  ]
}
```

### Prometheus Integration

Expose health check results as Prometheus metrics from your off-chain collector:

```python
from prometheus_client import Gauge

# Gauge: 1 = Healthy, 0.5 = Degraded, 0 = Down
health_level = Gauge('qc_health_level', 'Contract health level (1=Healthy, 0.5=Degraded, 0=Down)')
pubsub_up    = Gauge('qc_pubsub_connected', 'PubSub bus connectivity (1=up, 0=down)')
revstore_up  = Gauge('qc_revocation_store_connected', 'RevocationStore connectivity')
webhook_up   = Gauge('qc_webhook_registry_connected', 'WebhookRegistry connectivity')
yield_solvent = Gauge('qc_yield_reserve_solvent', 'Yield reserve solvency (1=solvent)')

def update_health_metrics(health_response: dict):
    status = health_response['overall_status']
    health_level.set(1.0 if status == 'Healthy' else (0.5 if status == 'Degraded' else 0.0))
    pubsub_up.set(1 if health_response['pubsub_connected'] else 0)
    revstore_up.set(1 if health_response['revocation_store_connected'] else 0)
    webhook_up.set(1 if health_response['webhook_registry_connected'] else 0)
    yield_solvent.set(1 if health_response['yield_reserve_solvent'] else 0)
```

### Alert Rules

Add to your existing `alerts.yml`:

```yaml
groups:
  - name: quorum-credit-health
    rules:
      # Page: contract is down
      - alert: ContractDown
        expr: qc_health_level == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "QuorumCredit contract is DOWN"
          description: "Health check returned Down status. Immediate investigation required."

      # Warning: degraded for more than 5 minutes
      - alert: ContractDegraded
        expr: qc_health_level == 0.5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "QuorumCredit contract is DEGRADED"
          description: "Non-critical sub-systems unavailable for >5m. Contract serving via fallbacks."

      # Warning: yield reserve getting low
      - alert: YieldReserveInsufficient
        expr: qc_yield_reserve_solvent == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Yield reserve is insufficient"
          description: "Contract token balance is below 1 XLM. Repayments will fail."

      # Warning: PubSub disconnected for extended period
      - alert: PubSubDisconnected
        expr: qc_pubsub_connected == 0
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "PubSub bus disconnected for >15m"
          description: "Event delivery is running on local fallback. Check the relay process."
```

- [Event Indexing Guide](./event-indexing-guide.md) — full event schema and indexer documentation
- [tools/indexer/](../tools/indexer/) — indexer source code and integration tests
