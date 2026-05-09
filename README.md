# Bevy Sim

Copper Island is a small data-driven economy simulator used to learn Rust, Bevy ECS, and the production/logistics ideas that will eventually support a real-world economy model.

## Architecture

```text
canonical JSON data
        -> sim_data validation/loading
        -> sim_core deterministic simulation
        -> bevy_client rendering, input, debug UI
```

The Python/Prefect/Postgres ingestion work is intentionally outside this repo for now. This repo owns the canonical game schema, pure simulation rules, and Bevy learning harness.

## Crates

- `crates/sim_core`: no Bevy dependency; recipes, inventories, facilities, ticks, graph lookups, and logistics.
- `crates/sim_data`: serde schema, canonical JSON loading, and validation.
- `crates/bevy_client`: Copper Island Bevy app with map, selection, debug panels, route drawing, and simple build commands.

## First Run

Install Rust and Bevy's Linux dependencies from [docs/dev_setup.md](docs/dev_setup.md), then run:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p bevy_client
```

The first Bevy compile can take a while.

The schema shape is summarized in [docs/canonical_schema.md](docs/canonical_schema.md), and the commit/review rhythm is in [docs/workflow.md](docs/workflow.md).

You can inspect the canonical tutorial data without launching Bevy:

```bash
cargo run -p sim_data --bin economy_inspect -- scenario
cargo run -p sim_data --bin economy_inspect -- commodity component.copper_wire
cargo run -p sim_data --bin economy_inspect -- recipe recipe.draw_copper_wire.v1
```

## Copper Island Prototype

Goal:

```text
Produce 100 electricity and 25 copper wire.
```

The client starts with settlement inventory and an island containing forest, coal, copper, iron, limestone, buildable land, and water tiles. Select tiles with the mouse. Build commands are intentionally simple while the ECS shape is still forming:

- `1`: camp on forest
- `2`: mine or quarry on coal, copper, iron, or limestone
- `3`: heat furnace on settlement
- `4`: copper furnace on settlement
- `5`: generator on settlement
- `6`: wire workshop on settlement
- `7`: warehouse on settlement or buildable ground
- `Tab`: cycle recipe graph commodity
- `R`: cycle selected logistics route
- `=` / `-`: increase or decrease selected route capacity

Resource extraction facilities output to their tile node. A low-capacity route moves goods back to the settlement, which makes transport bottlenecks visible before individual vehicles exist.

The starter inventory, win condition, and build options now live in `data/canonical/v0/scenarios.json`.
