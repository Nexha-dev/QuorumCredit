# QuorumCredit Monitoring Setup Guide

Complete step-by-step guide to deploy and configure Prometheus, Grafana, and alerting for QuorumCredit protocol monitoring.

## Quick Summary

| Component | Purpose | Location |
|-----------|---------|----------|
| **QuorumCredit Indexer** | Event stream processor, metrics exporter | `tools/indexer/` |
| **Prometheus** | Metrics collection & storage | `/etc/prometheus/` |
| **Grafana** | Metrics visualization & dashboards | `http://localhost:3000` |
| **AlertManager** | Alert routing & notifications | `/etc/alertmanager/` |

**Estimated setup time:** 30-45 minutes  
**Required expertise:** Linux administration, basic networking  

---

## Prerequisites

- Linux server (Ubuntu 20.04+, CentOS 7+, or similar)
- 4GB+ RAM, 20GB+ disk space
- Internet access for downloading packages
- Port access: 9090 (Prometheus), 3000 (Grafana), 9093 (AlertManager), 9100 (Node Exporter)
- Stellar Soroban RPC endpoint (public or self-hosted)

---

## Step 1: Deploy the QuorumCredit Indexer

The indexer is your data source. It processes Soroban events and exposes Prometheus metrics.

### 1.1 Clone and Build

```bash
cd /opt
git clone https://github.com/your-org/QuorumCredit.git
cd QuorumCredit/tools/indexer

# Build in release mode
cargo build --release --bin quorum-credit-indexer

# Verify build
./target/release/quorum-credit-indexer --version
```

### 1.2 Configure Environment

```bash
# Create config directory
sudo mkdir -p /etc/quorum-credit-indexer
sudo chown $USER:$USER /etc/quorum-credit-indexer

# Create environment file
cat > /etc/quorum-credit-indexer/.env << 'EOF'
# Stellar Soroban RPC
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org

# Contract address (update for mainnet)
CONTRACT_ADDRESS=CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4

# Metrics server
METRICS_PORT=9090
METRICS_BIND=0.0.0.0

# Database
DATABASE_PATH=/data/indexer.db

# Logging
LOG_LEVEL=info
LOG_FORMAT=json

# Ledger processing
BATCH_SIZE=100
CHECKPOINT_INTERVAL=60
EOF

chmod 600 /etc/quorum-credit-indexer/.env
```

### 1.3 Create Data Directory

```bash
sudo mkdir -p /data/quorum-credit
sudo chown $USER:$USER /data/quorum-credit
chmod 700 /data/quorum-credit
```

### 1.4 Create systemd Service

```bash
sudo tee /etc/systemd/system/quorum-credit-indexer.service > /dev/null << 'EOF'
[Unit]
Description=QuorumCredit Indexer
After=network.target
StartLimitIntervalSec=60
StartLimitBurst=3

[Service]
Type=simple
User=quorum
WorkingDirectory=/opt/QuorumCredit/tools/indexer
EnvironmentFile=/etc/quorum-credit-indexer/.env
ExecStart=/opt/QuorumCredit/tools/indexer/target/release/quorum-credit-indexer
Restart=on-failure
RestartSec=10

# Security & Resource Limits
PrivateTmp=yes
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/data/quorum-credit /var/log/quorum-credit
MemoryMax=2G
CPUQuota=80%

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=quorum-credit-indexer

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable quorum-credit-indexer
sudo systemctl start quorum-credit-indexer

# Verify startup
sudo systemctl status quorum-credit-indexer
```

### 1.5 Test Metrics Endpoint

```bash
# Should return Prometheus metrics
curl -s http://localhost:9090/metrics | head -20

# Expected output:
# qc_indexer_ledger_height 12345
# qc_loan_count_total 42
# qc_attestor_active_stake_stroops{attestor="..."} 10000000
# ...
```

Attestor metrics are derived from indexed vouch events and reflect on-chain stake and activity, not node uptime or signing availability. The repository's Grafana provisioning includes an **Attestor Health** dashboard, and its Prometheus rules warn when an observed attestor has no active stake for five minutes.

---

## Step 2: Install Prometheus

### 2.1 Download & Install

```bash
# Get latest version
PROM_VERSION=2.45.0
wget https://github.com/prometheus/prometheus/releases/download/v${PROM_VERSION}/prometheus-${PROM_VERSION}.linux-amd64.tar.gz

# Extract
tar xzf prometheus-${PROM_VERSION}.linux-amd64.tar.gz
sudo mv prometheus-${PROM_VERSION}.linux-amd64 /opt/prometheus

# Create prometheus user
sudo useradd --no-create-home --shell /bin/false prometheus || true

# Set permissions
sudo chown -R prometheus:prometheus /opt/prometheus
sudo mkdir -p /var/lib/prometheus /etc/prometheus
sudo chown prometheus:prometheus /var/lib/prometheus /etc/prometheus
```

### 2.2 Deploy Configuration

```bash
# Copy Prometheus config
sudo cp docs/prometheus-config.yml /etc/prometheus/prometheus.yml
sudo chown prometheus:prometheus /etc/prometheus/prometheus.yml

# Copy alerting rules
sudo cp docs/prometheus-alerts.yml /etc/prometheus/alerts.yml
sudo chown prometheus:prometheus /etc/prometheus/alerts.yml

# Verify config syntax
/opt/prometheus/promtool check config /etc/prometheus/prometheus.yml
```

### 2.3 Create systemd Service

```bash
sudo tee /etc/systemd/system/prometheus.service > /dev/null << 'EOF'
[Unit]
Description=Prometheus
After=network.target

[Service]
Type=simple
User=prometheus
Group=prometheus
WorkingDirectory=/var/lib/prometheus
ExecStart=/opt/prometheus/prometheus \
    --config.file=/etc/prometheus/prometheus.yml \
    --storage.tsdb.path=/var/lib/prometheus \
    --storage.tsdb.retention.time=30d \
    --web.enable-lifecycle \
    --web.enable-admin-api
Restart=on-failure
RestartSec=10

# Security
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=true
PrivateTmp=yes
ReadWritePaths=/var/lib/prometheus

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=prometheus

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable prometheus
sudo systemctl start prometheus
sudo systemctl status prometheus
```

### 2.4 Test Prometheus

```bash
# Access web UI
# http://localhost:9090

# Verify targets are scraping
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[].labels | {job, instance}'

# Query metrics
curl -s 'http://localhost:9090/api/v1/query?query=qc_active_loans' | jq '.data.result'
```

---

## Step 3: Install Grafana

### 3.1 Download & Install

```bash
# Add Grafana repository
sudo apt-get install -y software-properties-common
sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -

# Install Grafana
sudo apt-get update
sudo apt-get install -y grafana-server

# Enable & start
sudo systemctl daemon-reload
sudo systemctl enable grafana-server
sudo systemctl start grafana-server
```

### 3.2 Configure Grafana

```bash
# Edit configuration (if needed)
sudo nano /etc/grafana/grafana.ini

# Key settings:
# [security]
# admin_password = <strong-password>
# [users]
# allow_sign_up = false
# [auth.anonymous]
# enabled = false
```

### 3.3 Add Prometheus Data Source

```bash
# Access Grafana
# http://localhost:3000
# Default: admin / admin (change password immediately!)

# Via API:
curl -X POST http://localhost:3000/api/datasources \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <your-api-token>" \
  -d '{
    "name": "Prometheus",
    "type": "prometheus",
    "url": "http://localhost:9090",
    "access": "proxy",
    "isDefault": true
  }'
```

### 3.4 Import Dashboards

**Option A: Via Grafana UI**

1. Navigate to `Dashboards → Import`
2. Upload JSON from `docs/grafana-dashboard-protocol-overview.json`
3. Repeat for additional dashboards

**Option B: Via API**

```bash
# Import protocol overview dashboard
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <api-token>" \
  -d @docs/grafana-dashboard-protocol-overview.json
```

---

## Step 4: Install AlertManager

### 4.1 Download & Install

```bash
ALERTMANAGER_VERSION=0.25.0
wget https://github.com/prometheus/alertmanager/releases/download/v${ALERTMANAGER_VERSION}/alertmanager-${ALERTMANAGER_VERSION}.linux-amd64.tar.gz

tar xzf alertmanager-${ALERTMANAGER_VERSION}.linux-amd64.tar.gz
sudo mv alertmanager-${ALERTMANAGER_VERSION}.linux-amd64 /opt/alertmanager

# Create alertmanager user
sudo useradd --no-create-home --shell /bin/false alertmanager || true

# Set permissions
sudo chown -R alertmanager:alertmanager /opt/alertmanager
sudo mkdir -p /var/lib/alertmanager /etc/alertmanager
sudo chown alertmanager:alertmanager /var/lib/alertmanager /etc/alertmanager
```

### 4.2 Configure AlertManager

```bash
# Create config
sudo tee /etc/alertmanager/alertmanager.yml > /dev/null << 'EOF'
global:
  resolve_timeout: 5m
  slack_api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'

route:
  receiver: 'default'
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  routes:
    - match:
        severity: critical
      receiver: 'critical'
      repeat_interval: 5m
    - match:
        severity: warning
      receiver: 'warning'
      repeat_interval: 1h

receivers:
  - name: 'default'
    slack_configs:
      - channel: '#alerts-general'
        title: 'Alert: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
  
  - name: 'critical'
    slack_configs:
      - channel: '#alerts-critical'
        title: '🚨 CRITICAL: {{ .GroupLabels.alertname }}'
    pagerduty_configs:
      - service_key: 'YOUR-PAGERDUTY-KEY'
  
  - name: 'warning'
    slack_configs:
      - channel: '#alerts-warnings'
        title: '⚠️ WARNING: {{ .GroupLabels.alertname }}'

inhibit_rules:
  - source_match:
      severity: 'critical'
    target_match:
      severity: 'warning'
    equal: ['alertname', 'service']
EOF

sudo chown alertmanager:alertmanager /etc/alertmanager/alertmanager.yml
```

### 4.3 Create systemd Service

```bash
sudo tee /etc/systemd/system/alertmanager.service > /dev/null << 'EOF'
[Unit]
Description=Alertmanager
After=network.target

[Service]
Type=simple
User=alertmanager
Group=alertmanager
WorkingDirectory=/var/lib/alertmanager
ExecStart=/opt/alertmanager/alertmanager \
    --config.file=/etc/alertmanager/alertmanager.yml \
    --storage.path=/var/lib/alertmanager \
    --web.external-url=http://localhost:9093
Restart=on-failure
RestartSec=10

ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=true
PrivateTmp=yes
ReadWritePaths=/var/lib/alertmanager

StandardOutput=journal
StandardError=journal
SyslogIdentifier=alertmanager

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable alertmanager
sudo systemctl start alertmanager
```

---

## Step 5: Deploy Node Exporter (Optional)

For infrastructure monitoring (CPU, memory, disk, network):

```bash
# Download
NODE_EXPORTER_VERSION=1.6.1
wget https://github.com/prometheus/node_exporter/releases/download/v${NODE_EXPORTER_VERSION}/node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64.tar.gz

tar xzf node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64.tar.gz
sudo mv node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64/node_exporter /usr/local/bin/

# Create service
sudo tee /etc/systemd/system/node_exporter.service > /dev/null << 'EOF'
[Unit]
Description=Node Exporter
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/node_exporter

Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable node_exporter
sudo systemctl start node_exporter
```

---

## Step 6: Verification Checklist

```bash
# 1. Check all services running
sudo systemctl status quorum-credit-indexer prometheus alertmanager grafana-server node_exporter

# 2. Verify metrics endpoints
curl -s http://localhost:9090/metrics | wc -l      # Prometheus
curl -s http://localhost:9090/metrics | wc -l      # Indexer
curl -s http://localhost:9100/metrics | wc -l      # Node Exporter

# 3. Check Prometheus targets
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets | length'

# 4. Test queries
curl -s 'http://localhost:9090/api/v1/query?query=qc_loan_count_total'

# 5. Access dashboards
# Grafana: http://localhost:3000
# Prometheus: http://localhost:9090
# AlertManager: http://localhost:9093
```

---

## Step 7: Configure Backup & Retention

### 7.1 Prometheus Data Retention

Already configured in systemd service (30 days). Adjust as needed:

```bash
# Change retention to 60 days
sudo systemctl edit prometheus
# Add to [Service]: ExecStart parameter --storage.tsdb.retention.time=60d
sudo systemctl restart prometheus
```

### 7.2 Backup Strategy

```bash
# Backup Prometheus data weekly
cat > /opt/backup-prometheus.sh << 'EOF'
#!/bin/bash
BACKUP_DIR=/backup/prometheus
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# Snapshot Prometheus
curl -s http://localhost:9090/api/v1/admin/tsdb/snapshot
SNAPSHOT=$(curl -s http://localhost:9090/api/v1/admin/tsdb/snapshot | jq -r '.data.name')

# Compress and upload
tar czf $BACKUP_DIR/prometheus_${TIMESTAMP}.tar.gz /var/lib/prometheus/snapshots/$SNAPSHOT

# Clean old snapshots
find /var/lib/prometheus/snapshots -mtime +7 -delete
EOF

chmod +x /opt/backup-prometheus.sh

# Schedule with cron (weekly on Sundays)
echo "0 2 * * 0 /opt/backup-prometheus.sh" | sudo crontab -
```

---

## Step 8: Security Hardening

### 8.1 Firewall Rules

```bash
# Allow local access only (production best practice)
sudo ufw allow from 127.0.0.1 to any port 9090  # Prometheus
sudo ufw allow from 127.0.0.1 to any port 3000  # Grafana
sudo ufw allow from 127.0.0.1 to any port 9093  # AlertManager

# Or expose via reverse proxy (nginx/apache) with auth
```

### 8.2 Reverse Proxy (Nginx)

```nginx
server {
    listen 443 ssl http2;
    server_name grafana.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/grafana.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/grafana.yourdomain.com/privkey.pem;

    auth_basic "QuorumCredit Monitoring";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

---

## Troubleshooting

### Issue: Indexer Metrics Not Appearing

```bash
# Check indexer logs
sudo journalctl -u quorum-credit-indexer -n 50

# Verify metrics endpoint
curl -v http://localhost:9090/metrics

# Check Prometheus scrape errors
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.health == "down")'
```

### Issue: Prometheus Not Scraping

```bash
# Reload config
curl -X POST http://localhost:9090/-/reload

# Check Prometheus logs
sudo journalctl -u prometheus -n 50

# Verify config syntax
/opt/prometheus/promtool check config /etc/prometheus/prometheus.yml
```

### Issue: Grafana Slow/Memory Leak

```bash
# Check Grafana logs
sudo journalctl -u grafana-server -n 100

# Restart Grafana
sudo systemctl restart grafana-server

# Increase memory limit if needed
sudo systemctl edit grafana-server
# Add: MemoryMax=4G
```

---

## Maintenance

### Monthly Tasks

- [ ] Review disk usage: `df -h /var/lib/prometheus`
- [ ] Verify backups completed: `ls -lh /backup/prometheus`
- [ ] Check alert accuracy (false positives?)
- [ ] Update alert thresholds based on trends
- [ ] Review metrics retention settings

### Quarterly Tasks

- [ ] Update Prometheus & Grafana versions
- [ ] Audit access logs
- [ ] Test disaster recovery procedure
- [ ] Prune old data if needed
- [ ] Update documentation

---

## References

- [Prometheus Docs](https://prometheus.io/docs/)
- [Grafana Docs](https://grafana.com/docs/)
- [AlertManager Docs](https://prometheus.io/docs/alerting/latest/overview/)
- [QuorumCredit Monitoring Guide](./monitoring-guide.md)
- [Prometheus Config Examples](./prometheus-config.yml)
- [Alert Rules](./prometheus-alerts.yml)

---

## Support

For issues or questions:

1. Check logs: `sudo journalctl -u <service> -n 100`
2. Verify metrics: `curl http://localhost:9090/metrics`
3. Review documentation above
4. Open issue: https://github.com/your-org/QuorumCredit/issues
5. Email: monitoring-team@yourdomain.com
