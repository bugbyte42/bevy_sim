# Canonical Economy Schema v0

The canonical schema is deliberately small and game-facing. External ingestion can be messy, but Bevy systems should only consume this normalized shape.

## Files

```text
data/canonical/v0/commodities.json
data/canonical/v0/recipes.json
data/canonical/v0/facilities.json
data/canonical/v0/regions.json
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

## Extension Direction

Prefer adding optional fields or new versioned files over changing existing meanings. The expected future path is:

```text
v0 hand-authored tutorial data
v1 curated mineral/food/energy regional profiles
v2 provenance-rich ingestion outputs from the external Python pipeline
```

The Rust boundary should remain stable: `sim_data` validates canonical entities, then converts recipes into `sim_core::RecipeBook`.
