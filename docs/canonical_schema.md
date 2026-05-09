# Canonical Economy Schema v0

The canonical schema is deliberately small and game-facing. External ingestion can be messy, but Bevy systems should only consume this normalized shape.

## Files

```text
data/canonical/v0/commodities.json
data/canonical/v0/recipes.json
data/canonical/v0/facilities.json
data/canonical/v0/regions.json
data/canonical/v0/scenarios.json
```

## Shared Metadata

Every canonical entity carries:

- `id`: stable namespaced string, such as `ore.copper`.
- `display_name`: UI-facing name.
- `tags`: flexible grouping hints for graph, UI, and facility matching.
- `source_refs`: provenance records; empty is allowed only for hand-authored tutorial data.
- `confidence`: `high`, `medium`, or `low`.
- `authored_status`: `hand_authored`, `draft`, `llm_candidate`, `reviewed`, or `trusted`.

## Quantity Policy

Phase 1 uses abstract `unit` quantities and `f64` values. That keeps recipes readable while leaving room for later canonical units such as kg, kWh, tonne-km, or worker-hours.

The validator rejects:

- unknown commodity IDs,
- negative quantities,
- zero-duration recipes,
- recipes with no outputs,
- invalid facility recipe patterns,
- invalid regional abundance values.
- scenario references to unknown regions, facilities, recipes, commodities, or tile kinds.
- malformed scenario map layouts, including ragged rows, unknown tile kinds, missing settlement tiles, or invalid initial selections.

## Scenario Data

Scenarios keep tutorial/gameplay setup out of Bevy client code:

- `map_layout`: a compact row-based tile layout plus initial selected tile.
- `starting_inventory`: what the settlement begins with.
- `win_conditions`: target commodities and whether progress is measured by produced totals or current inventory.
- `build_options`: key binding, label, facility archetype, active recipe, allowed tile kinds, optional transported output, and whether the facility lives on the selected tile node or settlement node.

This is still intentionally simple. The first goal is not a general campaign system; it is a small data seam that lets future scenarios grow without recompiling client logic for every build option.

## Extension Direction

Prefer adding optional fields or new versioned files over changing existing meanings. The expected future path is:

```text
v0 hand-authored tutorial data
v1 curated mineral/food/energy regional profiles
v2 provenance-rich ingestion outputs from the external Python pipeline
```

The Rust boundary should remain stable: `sim_data` validates canonical entities, then converts recipes into `sim_core::RecipeBook`.

## Inspection

Use the headless inspector when changing canonical data:

```bash
cargo run -p sim_data --bin economy_inspect -- scenario
cargo run -p sim_data --bin economy_inspect -- list-scenarios
cargo run -p sim_data --bin economy_inspect -- map scenario.copper_island.logistics_squeeze
cargo run -p sim_data --bin economy_inspect -- commodity component.copper_wire
cargo run -p sim_data --bin economy_inspect -- recipe recipe.draw_copper_wire.v1
```

This is intentionally lightweight: it is a quick review aid, not a replacement for the Bevy client.
