# Canonical Economy Schema v0

The canonical schema is deliberately small and game-facing. External ingestion can be messy, but Bevy systems should only consume this normalized shape.

## Files

```text
data/canonical/v0/commodities.json
data/canonical/v0/recipes.json
data/canonical/v0/facilities.json
data/canonical/v0/regions.json
data/canonical/v0/world_regions.json
data/canonical/v0/world_resource_profiles.json
data/canonical/v0/scenarios.json
data/canonical/v0/world_scenarios.json
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
- `description`: optional player-facing summary for scenario pickers and inspectors.
- `objective_notes`: optional ordered player-facing hints; `win_conditions` remain the actual completion source of truth.
- `starting_inventory`: what the settlement begins with.
- `win_conditions`: target commodities and whether progress is measured by produced totals or current inventory.
- `build_options`: key binding, label, facility archetype, active recipe, allowed tile kinds, optional transported output, and whether the facility lives on the selected tile node or settlement node.

This is still intentionally simple. The first goal is not a general campaign system; it is a small data seam that lets future scenarios grow without recompiling client logic for every build option.

## World Geometry

`world_regions.json` is the static Mini Earth geometry workbench. It is intentionally separate from `regions.json`: a world region is a drawable country/admin shape, while a gameplay region is a simulation profile.

Each world region includes:

- `id`: stable region id such as `world.usa`.
- `display_name` and `iso_a3`.
- `centroid_lon` and `centroid_lat`.
- `geometry`: simplified MultiPolygon rings as lon/lat pairs.
- shared metadata: `tags`, `source_refs`, `confidence`, and `authored_status`.

Regenerate it from a manually downloaded Natural Earth-style GeoJSON file with:

```bash
python tools/python/prepare_world_geometry.py \
  --input ../natural-earth-geojson/110m/cultural/ne_110m_admin_0_countries.json \
  --output data/canonical/v0/world_regions.json
```

The Bevy client can render this file as a standalone workbench without running the island economy:

```bash
BEVY_SIM_VIEW=world cargo run -p bevy_client
```

Or as a plain 3D globe viewer:

```bash
BEVY_SIM_VIEW=globe cargo run -p bevy_client
```

## Mini Earth Corridor Data

`world_resource_profiles.json` adds hand-authored static resource/demand profiles to a small set of world regions. These are deliberately lightweight bridge records, not a claim that the whole world economy is modeled.

`world_scenarios.json` defines selected corridor simulations over real map regions:

- `nodes`: named simulation nodes mapped to `world_regions.json` ids.
- `starting_inventory`: per-node initial stock.
- `facilities`: prewired pure-sim facilities and active recipes.
- `routes`: prewired transport edges and orders.
- `win_conditions`: produced or stocked targets, using the same commodity ids as the island economy.

The first corridor is `world_scenario.mini_earth.electrification_corridor`, which moves copper ore, coal, and timber into a demand node and produces electricity plus copper wire. The full world remains visual context; the simulation only runs on those selected nodes.

World-mode scenario selection uses `BEVY_WORLD_SCENARIO`; if it is unset, the electrification corridor is used.

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
cargo run -p sim_data --bin economy_inspect -- scenario scenario.copper_island.steel_gate
cargo run -p sim_data --bin economy_inspect -- world-map
cargo run -p sim_data --bin economy_inspect -- world-scenario
cargo run -p sim_data --bin economy_inspect -- commodity component.copper_wire
cargo run -p sim_data --bin economy_inspect -- recipe recipe.draw_copper_wire.v1
```

This is intentionally lightweight: it is a quick review aid, not a replacement for the Bevy client.
