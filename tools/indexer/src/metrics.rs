use crate::db::Event;
use prometheus::{
    core::Collector,
    Counter, CounterVec, Gauge, GaugeVec, Opts, Registry,
};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct IndexerMetrics {
    pub ledger_height: Gauge,
    pub events_total: CounterVec,
    pub loan_volume_total: CounterVec,
    pub loan_count_total: Counter,
    pub active_loans: Gauge,
    pub slash_events_total: Counter,
    pub slash_amount_total: CounterVec,
    pub vouch_count: Gauge,
    pub gaps_detected: Counter,
    pub reorgs_detected: Counter,
    pub errors_total: CounterVec,
    pub backfill_events_total: Counter,
    pub attestor_active_vouches: GaugeVec,
    pub attestor_active_stake_stroops: GaugeVec,
    pub attestor_last_vouch_ledger: GaugeVec,
    pub attestor_vouch_events_total: CounterVec,
    attestor_state: Mutex<AttestorState>,
    pub registry: Registry,
}

#[derive(Default)]
struct AttestorState {
    vouches: HashMap<(String, String), f64>,
    active_vouches: HashMap<String, f64>,
    active_stake: HashMap<String, f64>,
}

fn register<C: Collector + Clone>(registry: &Registry, c: C) -> C {
    registry
        .register(Box::new(c.clone()))
        .expect("metric registration failed");
    c
}

impl IndexerMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let ledger_height = register(
            &registry,
            Gauge::new("qc_indexer_ledger_height", "Last processed ledger sequence").unwrap(),
        );

        let events_total = register(
            &registry,
            CounterVec::new(
                Opts::new("qc_indexer_events_total", "Total events indexed"),
                &["category", "action"],
            )
            .unwrap(),
        );

        let loan_volume_total = register(
            &registry,
            CounterVec::new(
                Opts::new("qc_loan_volume_total", "Total loan amount disbursed (stroops)"),
                &["token"],
            )
            .unwrap(),
        );

        let loan_count_total = register(
            &registry,
            Counter::new("qc_loan_count_total", "Total loans created").unwrap(),
        );

        let active_loans = register(
            &registry,
            Gauge::new("qc_active_loans", "Current active loans").unwrap(),
        );

        let slash_events_total = register(
            &registry,
            Counter::new("qc_slash_events_total", "Total slash events").unwrap(),
        );

        let slash_amount_total = register(
            &registry,
            CounterVec::new(
                Opts::new("qc_slash_amount_total", "Total amount slashed (stroops)"),
                &["token"],
            )
            .unwrap(),
        );

        let vouch_count = register(
            &registry,
            Gauge::new("qc_vouch_count", "Total active vouches").unwrap(),
        );

        let gaps_detected = register(
            &registry,
            Counter::new(
                "qc_indexer_gap_detected_total",
                "Total retention-window gaps detected",
            )
            .unwrap(),
        );

        let reorgs_detected = register(
            &registry,
            Counter::new(
                "qc_indexer_reorgs_detected_total",
                "Total ledger reorgs detected",
            )
            .unwrap(),
        );

        let errors_total = register(
            &registry,
            CounterVec::new(
                Opts::new("qc_indexer_errors_total", "Indexer errors"),
                &["error_code"],
            )
            .unwrap(),
        );

        let backfill_events_total = register(
            &registry,
            Counter::new(
                "qc_indexer_backfill_events_total",
                "Total events indexed during backfill",
            )
            .unwrap(),
        );

        let attestor_active_vouches = register(
            &registry,
            GaugeVec::new(
                Opts::new("qc_attestor_active_vouches", "Active borrower vouches by attestor"),
                &["attestor"],
            )
            .unwrap(),
        );

        let attestor_active_stake_stroops = register(
            &registry,
            GaugeVec::new(
                Opts::new(
                    "qc_attestor_active_stake_stroops",
                    "Active vouch stake by attestor in stroops",
                ),
                &["attestor"],
            )
            .unwrap(),
        );

        let attestor_last_vouch_ledger = register(
            &registry,
            GaugeVec::new(
                Opts::new(
                    "qc_attestor_last_vouch_ledger",
                    "Ledger of the latest indexed vouch event by attestor",
                ),
                &["attestor"],
            )
            .unwrap(),
        );

        let attestor_vouch_events_total = register(
            &registry,
            CounterVec::new(
                Opts::new(
                    "qc_attestor_vouch_events_total",
                    "Indexed vouch events by attestor and action",
                ),
                &["attestor", "action"],
            )
            .unwrap(),
        );

        Self {
            ledger_height,
            events_total,
            loan_volume_total,
            loan_count_total,
            active_loans,
            slash_events_total,
            slash_amount_total,
            vouch_count,
            gaps_detected,
            reorgs_detected,
            errors_total,
            backfill_events_total,
            attestor_active_vouches,
            attestor_active_stake_stroops,
            attestor_last_vouch_ledger,
            attestor_vouch_events_total,
            attestor_state: Mutex::new(AttestorState::default()),
            registry,
        }
    }

    pub fn rebuild_from_events(&self, events: &[Event]) {
        self.attestor_active_vouches.reset();
        self.attestor_active_stake_stroops.reset();
        self.attestor_last_vouch_ledger.reset();
        self.attestor_vouch_events_total.reset();
        *self.attestor_state.lock().unwrap() = AttestorState::default();

        let mut event_counts: HashMap<(String, String), f64> = HashMap::new();

        for ev in events {
            self.record_attestor_event(ev);
            *event_counts
                .entry((ev.category.clone(), ev.action.clone()))
                .or_insert(0.0) += 1.0;
        }

        for ((cat, act), cnt) in &event_counts {
            self.events_total
                .with_label_values(&[cat, act])
                .inc_by(*cnt);
        }

        let mut max_ledger: f64 = 0.0;
        let mut loan_count: f64 = 0.0;
        let mut active_loans_val: f64 = 0.0;
        let mut vouch_count_val: f64 = 0.0;
        let mut slash_count: f64 = 0.0;
        let mut loan_volume: f64 = 0.0;
        let mut slash_amount: f64 = 0.0;

        for ev in events {
            max_ledger = max_ledger.max(ev.ledger as f64);

            let val: serde_json::Value =
                serde_json::from_str(&ev.value_json).unwrap_or(serde_json::Value::Null);

            match (ev.category.as_str(), ev.action.as_str()) {
                ("loan", "request") => {
                    loan_count += 1.0;
                    active_loans_val += 1.0;
                    if let Some(amount) = val.get("amount_stroops").and_then(|v| v.as_f64()) {
                        loan_volume += amount;
                    }
                }
                ("loan", "repay") => {
                    active_loans_val -= 1.0;
                }
                ("vouch", "create") => {
                    vouch_count_val += 1.0;
                }
                ("vouch", "withdraw") => {
                    vouch_count_val -= 1.0;
                }
                ("loan", "slash") => {
                    slash_count += 1.0;
                    if let Some(amount) = val.get("total_slashed_stroops").and_then(|v| v.as_f64())
                    {
                        slash_amount += amount;
                    }
                }
                _ => {}
            }
        }

        self.ledger_height.set(max_ledger);
        self.loan_count_total.inc_by(loan_count);
        self.active_loans.set(active_loans_val.max(0.0));
        self.vouch_count.set(vouch_count_val.max(0.0));
        self.slash_events_total.inc_by(slash_count);

        let loan_vol_token = Self::most_common_token(events, "loan");
        self.loan_volume_total
            .with_label_values(&[&loan_vol_token])
            .inc_by(loan_volume);

        let slash_token = Self::most_common_token(events, "loan");
        self.slash_amount_total
            .with_label_values(&[&slash_token])
            .inc_by(slash_amount);
    }

    fn most_common_token(events: &[Event], category: &str) -> String {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for ev in events {
            if ev.category != category {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&ev.value_json) {
                if let Some(token) = val.get("token").and_then(|v| v.as_str()) {
                    *counts.entry(token.to_string()).or_insert(0) += 1;
                }
            }
        }
        counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .map(|(t, _)| t)
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn record_event(&self, event: &Event) {
        self.ledger_height.set(event.ledger as f64);
        self.events_total
            .with_label_values(&[&event.category, &event.action])
            .inc();
        self.record_attestor_event(event);

        let val: serde_json::Value =
            serde_json::from_str(&event.value_json).unwrap_or(serde_json::Value::Null);

        match (event.category.as_str(), event.action.as_str()) {
            ("loan", "request") => {
                self.loan_count_total.inc();
                self.active_loans.inc();
                if let Some(amount) = val.get("amount_stroops").and_then(|v| v.as_f64()) {
                    let token = val
                        .get("token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    self.loan_volume_total
                        .with_label_values(&[token])
                        .inc_by(amount);
                }
            }
            ("loan", "repay") => {
                self.active_loans.dec();
            }
            ("loan", "slash") => {
                self.slash_events_total.inc();
                if let Some(amount) = val.get("total_slashed_stroops").and_then(|v| v.as_f64()) {
                    self.slash_amount_total
                        .with_label_values(&["unknown"])
                        .inc_by(amount);
                }
            }
            ("vouch", "create") => {
                self.vouch_count.inc();
            }
            ("vouch", "withdraw") => {
                self.vouch_count.dec();
            }
            _ => {}
        }
    }

    fn record_attestor_event(&self, event: &Event) {
        if event.category != "vouch"
            || !matches!(event.action.as_str(), "create" | "increase" | "decrease" | "withdraw")
        {
            return;
        }

        let value: serde_json::Value =
            serde_json::from_str(&event.value_json).unwrap_or(serde_json::Value::Null);
        let Some(attestor) = value.get("voucher").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(borrower) = value.get("borrower").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(event_stake) = value
            .get("stake_stroops")
            .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        else {
            return;
        };
        let event_stake = event_stake.max(0.0);

        let key = (attestor.to_string(), borrower.to_string());
        let mut state = self.attestor_state.lock().unwrap();
        let previous = state.vouches.get(&key).copied().unwrap_or(0.0);
        let current = match event.action.as_str() {
            "create" | "increase" => previous + event_stake,
            "decrease" => event_stake,
            "withdraw" => 0.0,
            _ => return,
        };

        let active_vouches = state.active_vouches.entry(attestor.to_string()).or_default();
        let active_stake = state.active_stake.entry(attestor.to_string()).or_default();
        if previous <= 0.0 && current > 0.0 {
            *active_vouches += 1.0;
        } else if previous > 0.0 && current <= 0.0 {
            *active_vouches = (*active_vouches - 1.0).max(0.0);
        }
        *active_stake = (*active_stake + current - previous).max(0.0);

        if current > 0.0 {
            state.vouches.insert(key, current);
        } else {
            state.vouches.remove(&key);
        }

        self.attestor_active_vouches
            .with_label_values(&[attestor])
            .set(*active_vouches);
        self.attestor_active_stake_stroops
            .with_label_values(&[attestor])
            .set(*active_stake);
        self.attestor_last_vouch_ledger
            .with_label_values(&[attestor])
            .set(event.ledger as f64);
        self.attestor_vouch_events_total
            .with_label_values(&[attestor, &event.action])
            .inc();
    }
}

#[cfg(test)]
mod tests {
    use super::IndexerMetrics;
    use crate::db::Event;

    fn vouch_event(ledger: u32, action: &str, stake: i64) -> Event {
        Event {
            id: None,
            ledger,
            ledger_closed_at: String::new(),
            tx_hash: String::new(),
            contract_id: String::new(),
            category: "vouch".to_string(),
            action: action.to_string(),
            value_json: serde_json::json!({
                "voucher": "GATT", "borrower": "GBOR", "stake_stroops": stake
            })
            .to_string(),
            raw_topics: None,
            raw_value: None,
        }
    }

    #[test]
    fn attestor_metrics_follow_vouch_lifecycle_and_rebuild() {
        let metrics = IndexerMetrics::new();
        let events = vec![
            vouch_event(10, "create", 100),
            vouch_event(11, "increase", 50),
            vouch_event(12, "decrease", 30),
        ];

        metrics.rebuild_from_events(&events);

        assert_eq!(metrics.attestor_active_vouches.with_label_values(&["GATT"]).get(), 1.0);
        assert_eq!(metrics.attestor_active_stake_stroops.with_label_values(&["GATT"]).get(), 30.0);
        assert_eq!(metrics.attestor_last_vouch_ledger.with_label_values(&["GATT"]).get(), 12.0);
        assert_eq!(
            metrics.attestor_vouch_events_total.with_label_values(&["GATT", "increase"]).get(),
            1.0
        );

        metrics.record_event(&vouch_event(13, "withdraw", 30));

        assert_eq!(metrics.attestor_active_vouches.with_label_values(&["GATT"]).get(), 0.0);
        assert_eq!(metrics.attestor_active_stake_stroops.with_label_values(&["GATT"]).get(), 0.0);
        assert_eq!(metrics.attestor_last_vouch_ledger.with_label_values(&["GATT"]).get(), 13.0);
    }
}
