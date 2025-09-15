# Privacy, UI, and Compliance Testing

This README summarizes how to execute tests/benches related to privacy redaction, UI snapshots, and compliance reporting.

- Redaction proptests (openagent-terminal-ai):
  - cargo test -p openagent-terminal-ai -- --nocapture
- UI buffer snapshots (openagent-terminal):
  - cargo test -p openagent-terminal ui_buffer
  - Update snapshots: INSTA_UPDATE=auto cargo test -p openagent-terminal ui_buffer
- Security Lens performance and compliance report stress:
  - cargo bench -p openagent-terminal security_analysis
  - cargo bench -p openagent-terminal compliance_report

Metrics exporter
- Set OPENAGENT_PROM_PORT=9898 (default) and start the terminal to expose Prometheus metrics at http://127.0.0.1:9898/metrics
