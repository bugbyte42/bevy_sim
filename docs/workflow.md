# Workflow

The project should grow in small, reviewable chunks. Codex can edit files and run short read/check commands, but git actions stay with the human lead.

## Review Rhythm

1. Implement one coherent chunk.
2. Run the short checks that are available locally.
3. Pause for review.
4. The human lead decides whether to commit, amend, pivot, or ask questions.

Suggested commit chunks:

- `docs: add local Rust Bevy setup runbook`
- `chore: create reproducible workspace harness`
- `chore: add VS Code workspace configuration`
- `feat(data): define v0 canonical economy schema`
- `feat(sim): implement pure recipe tick loop`
- `feat(client): render Copper Island debug map`
- `feat(gameplay): build Resource Garden loop`
- `feat(inspect): add recipe graph viewer`
- `feat(logistics): add node-edge transport`

## Manual Commands

Use these once Rust is installed:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
cargo run -p bevy_client
cargo run -p sim_data --bin economy_inspect -- scenario
cargo run -p sim_data --bin economy_inspect -- commodity component.copper_wire
```

Long-running commands, dependency downloads, and git commits are intentionally left for the human lead to run and monitor.

## Architecture Rule

Keep repeating the project boundary:

```text
Bevy renders and orchestrates.
sim_core simulates.
sim_data loads and validates canonical game data.
Python and Prefect ingestion live outside this repo for now.
```

That boundary is the main guardrail against turning early Bevy learning into a data-engineering tangle.
