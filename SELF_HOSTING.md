# Self-Hosting Guide for stream0

This guide covers how to deploy and operate stream0 in production environments.

## Quick Start

### Binary Installation

```bash
# Download the latest release
curl -L https://github.com/risingwavelabs/stream0/releases/latest/download/stream0 -o stream0
chmod +x stream0

# Run with default config
./stream0

# Or with custom config
./stream0 -config stream0.yaml
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STREAM0_SERVER_HOST` | Bind address | `127.0.0.1` |
| `STREAM0_SERVER_PORT` | Port | `8080` |
| `STREAM0_DB_PATH` | Database file path | `./stream0.db` |
| `STREAM0_LOG_LEVEL` | Log level | `info` |
| `STREAM0_LOG_FORMAT` | Log format (json/text) | `json` |

### Configuration File

Create `stream0.yaml`:

```yaml
server:
  host: 0.0.0.0
  port: 8080

database:
  path: /var/lib/stream0/stream0.db

log:
  level: info
  format: json
```

## Production Deployment

### Systemd Service

Create `/etc/systemd/system/stream0.service`:

```ini
[Unit]
Description=stream0 message bus
After=network.target

[Service]
Type=simple
User=stream0
Group=stream0
ExecStart=/usr/local/bin/stream0 -config /etc/stream0/stream0.yaml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Docker

```dockerfile
FROM alpine:latest
RUN apk add --no-cache ca-certificates
COPY stream0 /usr/local/bin/
EXPOSE 8080
CMD ["stream0"]
```

### Nginx Reverse Proxy

```nginx
upstream stream0 {
    server 127.0.0.1:8080;
}

server {
    listen 80;
    server_name stream0.example.com;

    location / {
        proxy_pass http://stream0;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

## Backup and Recovery

### Backup

stream0 uses SQLite, so backup is simple:

```bash
# While running (WAL mode allows this)
cp stream0.db stream0.db.backup

# Or use SQLite's online backup
sqlite3 stream0.db ".backup '/backup/stream0.db'"
```

### Recovery

```bash
# Stop stream0
systemctl stop stream0

# Restore backup
cp stream0.db.backup stream0.db

# Start stream0
systemctl start stream0
```

## Monitoring

### Health Check

```bash
curl http://localhost:8080/health
```

### Logging

stream0 outputs structured JSON logs:

```json
{
  "client_ip": "127.0.0.1",
  "timestamp": "2026-03-13T00:00:00Z",
  "method": "POST",
  "path": "/topics/tasks/messages",
  "status": 201,
  "latency": 1500000,
  "user_agent": "curl/7.64.1"
}
```

### Metrics (Future)

Metrics endpoint at `/metrics` (Prometheus format) is planned for v0.2.

## Troubleshooting

### Database locked

SQLite can show "database is locked" under high concurrency. stream0 uses WAL mode which handles most cases. If issues persist:

1. Increase SQLite busy timeout
2. Consider using PostgreSQL backend (future feature)

### High memory usage

SQLite caches can grow. Restart stream0 periodically or set:

```sql
PRAGMA cache_size = -64000;  -- 64MB cache limit
```

## Security

- **No authentication built-in** (planned for v0.2)
- Run behind reverse proxy for HTTPS
- Use firewall rules to limit access
- Consider VPN for agent communication

## Scaling

stream0 is single-node. For higher throughput:

1. **Vertical scaling**: More CPU/RAM on single node
2. **Sharding**: Run multiple stream0 instances, route by topic prefix
3. **External backend**: Use PostgreSQL instead of SQLite (v0.2)
