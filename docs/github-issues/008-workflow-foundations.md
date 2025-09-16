#008 — Workflow foundations (parser + UI)

Status: Open
Priority: High

Scope
- TOML/YAML workflow parser
- Basic parameterization
- Minimal launcher panel
- Persist workflow runs and re-run capability

References
- crates/workflow-engine/
- openagent-terminal/docs/workflows.md
- .github/workflows/ci.yml (integration features tests)

Acceptance criteria
- Parse/validate workflows with parameters
- Run workflows with UI input; persisted history; re-run works
