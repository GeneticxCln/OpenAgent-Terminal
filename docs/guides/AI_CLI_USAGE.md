# AI CLI Usage

This guide shows how to use the AI-related CLI commands in OpenAgent Terminal.

Storage locations (Linux)
- Base directory: ~/.local/share/openagent-terminal/ai_history/
- Files:
  - history.db (SQLite): primary store for AI conversation history
  - history.jsonl (JSON Lines): append-only log, one JSON object per line

Exporting history
- Export to JSON:
  openagent-terminal ai history-export --format json --to ./ai_history.json

- Export to CSV:
  openagent-terminal ai history-export --format csv --to ./ai_history.csv

Fallback behavior
- If the SQLite database cannot be opened (missing/locked/corrupt), the CLI automatically falls back to exporting from history.jsonl, skipping malformed lines.
- Exit code is 0 on success, 2 if no history is available.

Purging history
- Keep only the last N entries (SQLite):
  openagent-terminal ai history-purge --keep-last 500

- The purge command will also prune rotated JSONL files beyond the last 5.

Validating provider configuration
- Validate all (including defaults):
openagent-terminal ai validate --include-defaults

# Provider management

List providers (configured plus defaults):

  openagent-terminal ai provider list --include-defaults

Set the active provider (persists to config):

  openagent-terminal ai provider set openai

JSON output for scripting:

  openagent-terminal ai provider list --json

- Validate a specific provider (e.g., openai):
  openagent-terminal ai validate --provider openai

Migrating provider environment variables
- Generate provider sections and an optional env snippet:
  openagent-terminal ai migrate --config-out ~/.config/openagent-terminal/openagent-terminal.toml --apply --write-env-snippet ./ai_env.sh

Notes
- Do not commit secrets. Use environment variables. The CLI and providers read env securely.
- See docs/AI_ENVIRONMENT_SECURITY.md for details.