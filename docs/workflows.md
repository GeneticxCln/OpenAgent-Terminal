# Workflows

OpenAgent Terminal ships with a minimal workflow engine (crates/workflow-engine) for automating common tasks.

- Example workflow: examples/workflows/deploy.yaml
- Key concepts: steps, conditions, templating (Tera), variables, and validators

Quick start

1. Create a YAML file with your steps (build, test, deploy)
2. Run the workflow via the CLI or host integration (TBD)

Stability

- The workflow engine is considered experimental but aims for stable YAML schemas where possible
- Breaking changes to the schema will be noted in the changelog with migration guidance

Security considerations

- Treat workflows like scripts: review before running
- Use the Security Lens to validate generated shell commands

Samples
- See openagent-terminal/examples/workflows for curated templates:
  - rust.yaml
  - node.yaml
  - python.yaml

Dry-run support
- Each step supports `dry_run: true` to preview commands without executing them.
- Prefer dry-run gates before executing destructive operations

See also: docs/security_lens.md and crates/workflow-engine/ for API details
