#010 — Persistence (plugin storage + AI history)

Status: Open
Priority: High

Scope
- Namespaced plugin storage with quotas and isolation
- AI conversation history persistence (schema + tests)

References
- crates/plugin-api/ (storage preview)
- openagent-terminal/src/components_init.rs
- openagent-terminal/Cargo.toml (rusqlite/sqlx)

Acceptance criteria
- Storage APIs available to plugins with enforced quotas
- AI history persisted and queriable; tests cover CRUD and migration
